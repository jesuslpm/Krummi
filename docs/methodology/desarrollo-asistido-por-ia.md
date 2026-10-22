# Desarrollo práctico de software asistido por IA

Este documento sostiene una idea central: **el PRD es la fuente permanente de verdad funcional del
producto**. Las especificaciones técnicas, el diseño, las tareas y el código se derivan de él. Lo que cambia
de un enfoque a otro es cómo se ejecuta ese PRD: con vibe coding, con ODD o con SDD.

Todos los ejemplos vienen de un experimento real, el repositorio
[chicago-crime](https://github.com/jesuslpm/chicago-crime). Es una aplicación en Rust, un lenguaje que el
autor no conocía al empezar, que convierte 8,66 millones de delitos de Chicago en un conjunto de datos
consultable.

> **Aviso.** Lo que sigue son observaciones de un caso real y una propuesta de método, no una comparación
> controlada entre metodologías. Donde algo es una hipótesis, se dice.

## Contenido

1. [Introducción](#1-introducción)
2. [El PRD](#2-el-prd)
3. [Tres reglas antes de ejecutar](#3-tres-reglas-antes-de-ejecutar)
4. [Tres formas de ejecutar un PRD](#4-tres-formas-de-ejecutar-un-prd)
5. [Prompts para empezar](#5-prompts-para-empezar)
6. [El proceso vibe coding en detalle](#6-el-proceso-vibe-coding-en-detalle)
7. [El proceso SDD en detalle](#7-el-proceso-sdd-en-detalle)
8. [El proceso ODD en detalle](#8-el-proceso-odd-en-detalle)
9. [Bajo el capó: subagentes, memoria, skills y hooks](#9-bajo-el-capó-subagentes-memoria-skills-y-hooks)
10. [Desarrollo en paralelo con worktrees](#10-desarrollo-en-paralelo-con-worktrees)
11. [Evidencia del experimento](#11-evidencia-del-experimento)
12. [Qué sabemos y qué no](#12-qué-sabemos-y-qué-no)
13. [Referencias](#13-referencias)

---

## 1. Introducción

Muchos libros y cursos enseñan a programar con IA de dos maneras. La primera es el *vibe coding*, una
conversación informal con un modelo. La segunda es el *SDD* (Spec-Driven Development), una secuencia manual
de documentos: propuesta, especificaciones, diseño, tareas e implementación. Las dos sirven, pero dejan
fuera dos cosas que en un proyecto real importan mucho:

- **Quién decide qué hace el producto.** Las decisiones de negocio se acuerdan con una persona. Las de
  implementación se pueden delegar en los agentes, dentro de los límites aprobados.
- **Qué queda escrito cuando la conversación termina.** Un prompt se olvida. Un documento versionado en el
  repositorio lo puede leer cualquiera, incluso quien nunca va a abrir el código.

El planteamiento de este documento separa tres capas:

| Capa | Qué es | Carácter |
|---|---|---|
| **Requisitos** | El PRD, escrito para personas | Permanente |
| **Ejecución** | Vibe coding conversacional, ODD orgánico o SDD estructurado | Variable |
| **Evidencia** | Tests, criterios de aceptación, revisiones e informe de cierre | Imprescindible |

```text
Humano + IA → PRD → aprobación humana → enfoque de ejecución → verificación contra el PRD → informe de cierre
```

El mensaje no es que "la IA sustituye a los ingenieros" ni que "ya no hace falta aprender lenguajes". Es que
**un ingeniero que conoce el dominio, los algoritmos, la arquitectura y los invariantes puede dirigir agentes
para construir en tecnologías que todavía no domina**, sin dejar de ser responsable de especificar y de
evaluar el resultado.

---

## 2. El PRD

### 2.1 Qué es

Un PRD (*Product Requirements Document*) describe **qué** hace el producto y **por qué**, en lenguaje de
negocio. No es un prompt desechable para arrancar una sesión: es documentación duradera. La leen negocio,
usuarios, analistas y desarrolladores, y la usa el agente como contrato.

Lo que **no** lleva un PRD: nombres de clases, endpoints, rutas de archivos ni números de línea. Eso es el
*cómo*, y pertenece al documento de ejecución y al código.

Los PRD se organizan en jerarquía:

- **PRD padre**, uno por aplicación o módulo grande. Describe el producto completo. En el experimento:
  [chicago-crime.prd.md](../prd/chicago-crime/chicago-crime.prd.md).
- **PRD hijo**, uno por cambio. Nombra a su padre y aporta solo lo que ese cambio necesita. En el
  experimento: [generador](../prd/chicago-crime/generador.prd.md),
  [consultor](../prd/chicago-crime/consultor.prd.md) y
  [soporte Linux](../prd/chicago-crime/soporte-linux.prd.md).

Cuando un cambio contradice al padre, el padre no se reescribe: **se anota** con fecha, para conservar la
historia de la decisión.

### 2.2 Estructura

La plantilla del proyecto, [`prd-template.md`](../../.claude/skills/prd/assets/prd-template.md), tiene estas
secciones:

| Sección | Para qué sirve |
|---|---|
| Objetivo y contexto | Qué cambia, para quién y por qué ahora |
| Estado de partida | Lo que ya existe, **verificado** en el código y en los datos |
| Actores | Quién interviene y qué hace |
| Flujo | Los pasos desde el punto de vista del actor |
| Validaciones y mensajes | Cada situación, su regla y el mensaje exacto al usuario |
| Reutilización | Lo que ya existe y se aprovecha |
| Fuera de alcance | **Obligatorio.** La línea desde la que arranca el siguiente cambio |
| Criterios de aceptación | Verificables, en formato *Dado / Cuando / Entonces* |
| Decisiones | Cada decisión con su motivo y su consecuencia aceptada |

La sección **Decisiones** no estaba en la plantilla original. Se agregó durante el experimento porque
resultó la más útil: cierra cada pregunta de producto antes de programar y deja escrito por qué se decidió
así.

### 2.3 Cómo se produce

El PRD se escribe **conversando con la IA**, con un skill que le fija reglas
([`.claude/skills/prd`](../../.claude/skills/prd/SKILL.md), publicado también en
[este Gist](https://gist.github.com/jesuslpm/fc36d3503116171eee409b313365cde7)):

1. **Pedir el PRD y explicar el cambio.** El programador le pide a la IA un PRD para un cambio y le explica
   qué quiere y por qué, con el contexto que tenga (en el experimento, un resumen de una conversación previa
   con ChatGPT). El prompt de la sección 5.1 sirve de punto de partida.
2. **Leer** el PRD padre y la documentación del proyecto.
3. **Verificar el estado de partida** en el código y en los datos. En el experimento, el asistente recorrió el
   CSV completo antes de escribir una línea, y encontró que no venía ordenado, que el número de caso se
   repetía y que había 253 horas inexistentes por el cambio de horario. Las tres cosas cambiaron decisiones.
4. **Redactar un borrador** con la plantilla y dejar las decisiones abiertas, cada una **con su
   recomendación**.
5. **Cerrar las decisiones de una en una.** La IA pregunta solo lo que es genuinamente del usuario y nunca
   inventa una regla de negocio.
6. **Verificar antes de dar la razón.** Si el usuario afirma algo técnico, la IA lo comprueba en el código o
   en los datos antes de aceptarlo, y lo corrige con pruebas si no se sostiene.
7. **Iterar** hasta que no quede ninguna pregunta de producto abierta.
8. **Esperar la autorización.** Escribir el PRD no autoriza a implementar.

El ejemplo completo, con las preguntas, las respuestas literales y lo que se verificó en cada paso, está en
[Cómo se generó el PRD](generacion-del-prd.md).

### 2.4 Por qué importa

- **Separa decidir de construir.** Las decisiones de producto se toman con la persona que tiene autoridad
  para tomarlas, y quedan cerradas antes de escribir código. El agente puede trabajar horas sin detenerse,
  porque ya no le queda nada de negocio por preguntar.
- **Es un contrato verificable.** Los criterios de aceptación llevan cifras reales. En el experimento, el
  conteo de `THEFT` tenía que dar 1.838.502 y la aceptación lo comprobó sobre el CSV de 2,4 GB.
- **Limita el alcance.** "Fuera de alcance" impide que el agente amplíe el cambio por su cuenta y marca dónde
  empieza el siguiente.
- **Lo lee cualquiera.** Las especificaciones técnicas suelen ser densas y fragmentadas para un lector de
  negocio. El PRD cuenta el comportamiento con un flujo coherente y reglas legibles.
- **Dura más que el código.** Cuando cambia una regla, se cambia primero el PRD y después se trasladan sus
  consecuencias a las tareas, los tests y el código.
- **Hace auditable el trabajo con IA.** Con PRD, documento de ejecución, tests e informe de cierre, el
  experimento se puede revisar y compartir, algo que una demostración conversacional no permite.

### 2.5 Cuándo no hace falta

Las correcciones de bugs, las refactorizaciones y el tooling que **no cambian el comportamiento del
producto** pueden ir directo a la ejecución. El PRD es proporcional al cambio funcional, no una ceremonia
para cada commit.

---

## 3. Tres reglas antes de ejecutar

Valen para cualquiera de los tres enfoques, porque atacan los problemas que más tiempo hacen perder al
trabajar con agentes.

1. **Pedir una estimación de tiempo antes de empezar.** Cuando ya no quedan preguntas, el agente envía un
   mensaje que solo trae el rango para todo el trabajo y la pregunta de si empieza, y espera. Así la estimación
   no se pierde entre la actividad del agente. Cuando el programador confirma, anota la hora real de inicio. Al
   cerrar, compara el tiempo real con el estimado. Sin esa referencia no hay forma de calibrar ni de
   planificar. En el experimento, las estimaciones salieron altas: el consultor se estimó en 4–7 h y llevó unas
   3 h 15 min, y el soporte para Linux se estimó en 1,5–2,5 h y llevó unos 48 min.
2. **Que no haya interrupciones.** Todas las preguntas se hacen **al principio**, durante el PRD o antes de
   arrancar. Después, el agente no se detiene: ante una duda menor toma un valor razonable, lo documenta y lo
   informa al final. Tampoco pregunta si debe verificar, porque verifica siempre. Una sola pregunta a mitad de
   proceso puede dejar la ejecución parada durante horas mientras nadie está mirando la pantalla.
3. **Exigir un informe de cierre en Markdown, guardado en el repositorio.** El informe recoge:
   - el resultado y qué se verificó;
   - una fila por cada revisión automática, con sus hallazgos y cómo se resolvieron. Se anotan en el momento,
     porque Gentle AI no guarda los hallazgos una vez reconocida la revisión;
   - el tiempo real frente al estimado, con las esperas aparte, y cuánto tardó cada fase (exploración, código
     y tests, checks, aceptación, verificación, revisión RDD, correcciones, cierre). Las fases se anotan con un
     script al cambiar de una a otra, porque al final ya no se pueden reconstruir;
   - las líneas añadidas y eliminadas por área;
   - un resumen de un párrafo;
   - los valores elegidos sin preguntar y los pendientes.

   Ejemplos: [informe del consultor](../../odd/tasks/crime-query-report.md) e
   [informe del soporte Linux](../../odd/tasks/soporte-linux-report.md).

En el repositorio, estas reglas viven en tres piezas:

- **[`CLAUDE.md`](../../CLAUDE.md)**, las instrucciones del proyecto que Claude Code carga en cada sesión. Es
  deliberadamente corto: solo lleva las reglas que siempre aplican.
- **El skill [`closing-report`](../../.claude/skills/closing-report/SKILL.md)**, que solo se carga al cerrar.
  Tiene la [plantilla del informe](../../.claude/skills/closing-report/assets/closing-report-template.md) y un
  [script](../../.claude/skills/closing-report/scripts/numstat_by_area.py) que calcula las líneas por área a
  partir de `git`, para que el agente no tenga que sumarlas a mano. Otro
  [script](../../.claude/skills/closing-report/scripts/phase_times.py) anota cada cambio de fase durante la
  ejecución y, al cerrar, calcula el tiempo de cada fase y de cada commit, las esperas y las tareas en segundo
  plano, como la aceptación.
- **El skill [`vibe-coding`](../../.claude/skills/vibe-coding/SKILL.md)**, que aplica las tres reglas cuando
  se pide vibe coding. ODD y SDD ya traen su propio proceso; vibe coding no, y sin el skill dependería de que el
  prompt las pidiera.

Así las reglas dejan de depender de la memoria personal de quien trabaja: cualquiera que clone el repositorio
las hereda. `CLAUDE.md` solo lo lee Claude Code; otros agentes, como Codex u OpenCode, leen `AGENTS.md`.

---

## 4. Tres formas de ejecutar un PRD

En los tres casos el PRD es una fase previa y común. **No son tres frameworks equivalentes**: lo que cambia
es el mecanismo posterior de implementación, coordinación, verificación y persistencia.

### 4.1 PRD + vibe coding

Con el PRD aprobado, se implementa conversando directamente con la IA, paso a paso.

- **Sirve para:** prototipos, tareas acotadas, exploración y cambios sencillos.
- **Riesgo:** con cambios largos se pierde el contexto y los requisitos derivan. Que haya un PRD delante **no
  valida por sí solo** el vibe coding: siguen haciendo falta límites, tests y comprobaciones.
- **Cómo hacerlo bien:** aplicar las tres reglas de la sección 3, pedir tests de cada regla y un commit por
  paso, y contrastar el resultado con los criterios de aceptación del PRD. Una revisión externa de cada
  tramo, como la de RDD, cubre lo que el agente no ve de su propio código.

Lo detalla la sección 6.

### 4.2 PRD + SDD

Después del PRD, se siguen fases formales:

```text
propose → design → spec → apply → verify → archive
```

Cada fase produce un artefacto técnico. Herramientas como **OpenSpec**, **Spec Kit** y Gentle AI, antes de su
versión 4, lo automatizan.

- **Sirve para:** cambios de alto riesgo o complejidad, donde los artefactos técnicos formales se justifican.
- **Riesgo:** sobrecarga documental y de coordinación.
- **Experiencia del autor:** con un PRD bien cerrado, rara vez tenía que corregir lo que generaba el SDD, y
  podía dejar la implementación corriendo durante horas mientras avanzaba con otro cambio en un *worktree*
  aparte. Es una observación personal, no una medición.

La versión 4 de Gentle AI retiró el flujo SDD y lo reemplazó por ODD. Hay una
[solicitud abierta](https://github.com/Gentleman-Programming/gentle-ai/issues/5475) para que los dos convivan.

Lo detalla la sección 7.

### 4.3 PRD + ODD

ODD (*Organic Driven Development*) es el flujo predeterminado de Gentle AI. En lugar de fases fijas, adapta la
ceremonia al tamaño y al riesgo de cada cambio:

- explora el código;
- registra el trabajo grande en un documento de ejecución;
- implementa tarea por tarea, con un commit cada una;
- verifica en proporción al riesgo.

Lo detalla la sección 8.

### 4.4 Comparación

Es un **encuadre conceptual**, no el resultado de una comparación empírica.

| Aspecto | PRD + vibe coding | PRD + ODD | PRD + SDD |
|---|---|---|---|
| Fuente funcional | PRD aprobado | PRD aprobado | PRD aprobado |
| Después del PRD | Conversación directa e iterativa | Flujo orgánico: exploración, tareas, verificación proporcional | Fases formales de especificación, diseño y tareas |
| Formalidad técnica | Variable; depende de la disciplina | Adaptativa | Más estructurada |
| Coordinación humana | Mayor en cambios largos, salvo con prácticas explícitas | Baja: el agente ejecuta y deja progreso y evidencia | Ejecución sistemática y trazable por fases |
| Uso interesante | Prototipos, cambios sencillos | Funcionalidades reales con poca ceremonia | Cambios de alto riesgo o complejidad |
| Riesgo a vigilar | Deriva de requisitos, pérdida de contexto | Reinterpretar requisitos al pasarlos entre agentes | Sobrecarga documental |

En los tres casos, **el agente no altera en silencio un requisito funcional**. Si encuentra una decisión de
negocio abierta, vuelve al usuario.

---

## 5. Prompts para empezar

Son puntos de partida para copiar y adaptar. Repiten a propósito las tres reglas de la sección 3, aunque en
este repositorio ya estén en [`CLAUDE.md`](../../CLAUDE.md): así funcionan también en un proyecto que no lo
tenga. Lo que va entre `<…>` se reemplaza.

### 5.1 Escribir el PRD

```text
Vamos a escribir el PRD de <cambio> con el skill prd, tomando como padre <PRD padre o "ninguno">.
Verifica el estado de partida en el código y en los datos antes de redactar.
Se admiten todo tipo de sugerencias y preguntas: hazme todas las preguntas de producto ahora, de una en una
y con tu recomendación. Una sugerencia que acepte entra como decisión; una que rechace va a "Fuera de alcance".
No implementes nada.
```

Aquí no se pide estimación ni se prohíben las interrupciones: escribir el PRD **es** la conversación. La
invitación a sugerir funcionó en el experimento: de ahí salieron, entre otras cosas, el manifiesto, la
publicación atómica y la indexación de los valores vacíos.

### 5.2 Ejecutar con ODD

```text
Implementa <ruta del PRD> con ODD.
Antes de empezar, dame en un mensaje aparte una estimación de tiempo en rango y pregúntame si empiezas.
Cuando lo confirme, anota la hora real de inicio.
No me interrumpas: si surge una duda menor, elige un valor razonable, documéntalo y repórtalo al final.
Verifica siempre, sin preguntar.
Al cerrar, comprueba cada criterio de aceptación contra el PRD y guarda un informe de cierre en el repositorio.
```

### 5.3 Ejecutar con SDD

```text
Ejecuta el flujo SDD completo a partir de <ruta del PRD> con <OpenSpec | Spec Kit>, desde la propuesta
hasta la verificación incluida.
Antes de empezar, dame en un mensaje aparte una estimación de tiempo en rango y pregúntame si empiezas.
Cuando lo confirme, anota la hora real de inicio.
Sin interrupciones: las dudas menores se resuelven con valores por defecto documentados.
Al cerrar, comprueba cada criterio de aceptación contra el PRD y guarda un informe de cierre en el repositorio.
```

Este prompt necesita una herramienta de SDD instalada. Sin ella, el agente improvisa algo parecido a SDD, y
la comparación entre enfoques deja de ser justa.

### 5.4 Ejecutar con vibe coding

```text
Implementa lo que pide <ruta del PRD> usando vibe coding.
Antes de escribir, explora el código existente y reutiliza lo que ya haya: mantén el código DRY.
Escribe un test por cada criterio de aceptación y, al terminar, verifica cada criterio contra el PRD.
Si aparece una decisión de producto que el PRD no cubre, no la inventes: anótala y repórtala.
Antes de empezar, dame en un mensaje aparte una estimación de tiempo y pregúntame si empiezas; después,
trabaja sin interrupciones; al cerrar, guarda un informe de cierre en el repositorio con qué test cubre cada
criterio y si pasó.
```

Hay que nombrar el enfoque de forma explícita. Si el proyecto usa ODD por defecto, como este, un prompt que
no diga "vibe coding" termina ejecutándose con ODD.

En este repositorio basta con «Implementa `<ruta del PRD>` usando vibe coding.», porque el skill
[`vibe-coding`](../../.claude/skills/vibe-coding/SKILL.md) carga el resto de las instrucciones. El prompt
completo sirve para proyectos que no tienen el skill. El skill fija **los bordes** del trabajo: un mapa del
código existente, preguntas y estimación antes del primer cambio; código DRY, tests por criterio, verificación
contra el PRD e informe al final. Si RDD está activo, mantiene además su revisión automática, igual que en
ODD; la lanza Gentle AI sobre el diff de git. El skill no añade fases, documento de ejecución ni subagentes,
así que el vibe coding sigue siendo conversacional. Y la comparación entre enfoques es más justa: los tres cumplen las mismas tres reglas sin
depender de que el prompt se acuerde de pedirlas.

---

## 6. El proceso vibe coding en detalle

### 6.1 Qué es

Andrej Karpathy acuñó el término a comienzos de 2025. Describía una forma de programar en la que uno le dice
a la IA lo que quiere, acepta lo que genera, mira si el resultado funciona y casi se olvida del código.

En desarrollo profesional, la versión útil es menos ingenua. Es una **implementación conversacional**: el
humano y la IA avanzan juntos, paso a paso, y el humano revisa cada resultado. No hay un plan escrito ni fases
formales. El PRD hace de ancla y la conversación hace de plan.

### 6.2 El ciclo, partiendo de un PRD

1. **Arrancar con el prompt de la sección 5.4.** Le da a la IA el PRD, le pide la estimación de tiempo y le
   fija el mínimo de evidencia: un test por criterio y una verificación final contra el PRD. En este
   repositorio lo hace el skill `vibe-coding`.
2. **Explorar el estado de partida antes de escribir.** Para cada criterio, la IA busca en el código lo que ya
   existe: funciones, tipos, validaciones o mensajes parecidos. Clasifica cada criterio como *ya hecho*, *a
   medias*, *con código parecido para reutilizar* o *nuevo*, y comparte ese mapa en unas líneas. Sin este paso,
   el vibe coding escribe de cero lo que ya existía en otra carpeta, y el código acaba WET.
3. **Pedir un primer corte pequeño.** Por ejemplo, un flujo o un criterio de aceptación, de punta a punta,
   empezando por lo que el mapa dice que se puede reutilizar. Se ejecuta y se prueba enseguida.
4. **Iterar criterio a criterio.** Cada paso deja su test y un commit que sirve de punto de control. Antes de
   pasar al siguiente, se refactoriza para quitar la duplicación introducida, con los tests en verde. Si un
   paso sale mal, se vuelve al commit anterior en lugar de acumular parches.
5. **Revisión automática con RDD.** Si RDD está activo, tras cada commit Gentle AI evalúa el riesgo del diff,
   igual que en ODD: un commit de alto riesgo, o un tramo de unas 400 líneas, recibe una revisión externa con
   las lentes pertinentes, y sus hallazgos se resuelven antes del paso siguiente. Busca defectos que introdujo
   el cambio; el cumplimiento del PRD lo comprueba la verificación del paso 7. Al cerrar se revisa también
   el último tramo, aunque no llegue a ese tamaño, así que todo lo que se fusiona pasó por RDD. No necesita
   documento de ejecución: trabaja sobre el diff de git. El agente lo comprueba al empezar con
   `gentle-ai review mode status`. Si el comando no existe o falla, o si dice que RDD está desactivado, no
   hay revisión externa: el agente lo avisa, no lo instala ni lo activa, y el informe de cierre lo deja
   anotado.
6. **Corregir conversando.** El humano señala errores y pide ajustes. La IA no decide el producto: lo que el
   PRD no cubre se anota y se reporta, no se inventa.
7. **Verificar al final.** La IA ejecuta los tests, revisa el diff completo en busca de duplicación y comprueba
   cada criterio de aceptación contra el PRD, con comandos reales y sus resultados, no con un "ya funciona".
8. **Cerrar con el informe.** Con el skill `closing-report`, incluida la tabla que relaciona cada criterio con
   su test y su resultado, lo que se reutilizó, se extendió o se extrajo, y los hallazgos de las revisiones.

DRY significa que **cada regla vive en un solo sitio**, no que haya que unificar todo lo que se parece. Dos
funciones parecidas que cambian por motivos distintos deben seguir separadas; fusionarlas crea acoplamiento.

### 6.3 Qué deja escrito

Solo el código, los tests, los commits y el informe de cierre. **No hay documento de ejecución**, así que el
estado del trabajo vive en la conversación. Si la sesión se corta o el contexto se compacta, la única
continuidad es el PRD más el historial de git. Por eso los commits pequeños no son un detalle: son la memoria
del proceso.

### 6.4 Dónde falla y cómo evitarlo

| Problema | Cómo se manifiesta | Cómo evitarlo |
|---|---|---|
| Deriva de requisitos | La implementación se aleja del PRD sin que nadie lo note | Un test por criterio de aceptación y la verificación final contra el PRD |
| Pérdida de contexto | En sesiones largas, la IA olvida decisiones anteriores | Commits pequeños; retomar desde el PRD y `git log` |
| Código aceptado sin entender | Pasan los tests de hoy, pero nadie sabe por qué funciona | Revisar cada diff antes de aceptarlo |
| Verificación optimista | La IA afirma que funciona sin haberlo ejecutado | Exigir los comandos ejecutados y su salida |
| Alcance que crece | Se agregan cosas "ya que estamos" | El "Fuera de alcance" del PRD es el límite |
| Código duplicado (WET) | La IA reescribe lo que ya existía en otra parte | Mapa del estado de partida antes de escribir, buscar antes de crear y refactorizar tras cada verde |

### 6.5 Cuándo conviene otro enfoque

Si el cambio necesita varias sesiones, toca algo de alto riesgo (datos persistidos, seguridad, contratos que
otros consumen) o no se podría retomar solo con el PRD y el `git diff`, conviene pasar a ODD o SDD. Ahí el
progreso y la evidencia quedan escritos fuera de la conversación.

---

## 7. El proceso SDD en detalle

### 7.1 Qué es

En SDD (*Spec-Driven Development*), una especificación formal precede al código. El trabajo se divide en
fases, y cada una produce un artefacto que se puede revisar antes de pasar a la siguiente. La IA ejecuta las
fases y el humano revisa en los puntos de control.

### 7.2 Las fases

```text
propose → design → spec → apply → verify → archive
```

| Fase | Qué produce |
|---|---|
| `propose` | La propuesta: por qué hace falta el cambio y qué cambia |
| `design` | El enfoque técnico |
| `spec` | Requisitos de comportamiento con escenarios *cuando / entonces*, escritos como cambios (deltas) sobre las especificaciones vigentes |
| `apply` | La implementación, marcando cada tarea al terminarla |
| `verify` | La comprobación del resultado contra las especificaciones |
| `archive` | La integración de los deltas en las especificaciones vigentes, que quedan listas para el próximo cambio |

La lista de tareas se genera antes de `apply`, junto con el diseño y las especificaciones; en OpenSpec, por
ejemplo, `propose` crea todos esos artefactos de una vez. Muchas herramientas ofrecen además un `explore`
opcional previo, para leer el código y las opciones antes de proponer nada.

### 7.3 Cómo lo implementan las herramientas

- **[OpenSpec](https://github.com/Fission-AI/OpenSpec).** Se instala con `openspec init` y guarda cada cambio
  en `openspec/changes/<cambio>/`, con `proposal.md`, `design.md`, `tasks.md` y los deltas en `specs/`. Los
  comandos son `/opsx:explore`, `/opsx:propose <cambio>`, `/opsx:apply` y `/opsx:archive`. **`/opsx:verify`
  solo está en el perfil ampliado**: para un flujo "hasta la verificación incluida", hay que activarlo con
  `openspec config profile` o pedir la verificación en el prompt.
- **[Spec Kit](https://github.com/github/spec-kit)**, de GitHub. Usa `/speckit-constitution`, una vez por
  proyecto, y después `/speckit-specify`, `/speckit-plan`, `/speckit-tasks` y `/speckit-implement`. Por último,
  `/speckit-converge` se repite junto con `/speckit-implement` hasta que informa que el resultado convergió.
- **Gentle AI antes de la versión 4.** Seguía las mismas fases: un orquestador delegaba cada una en un
  subagente y guardaba los artefactos de cada fase.

Los nombres de los comandos cambian entre versiones. Conviene revisar la documentación de la herramienta antes
de la demo.

### 7.4 La relación con el PRD

El PRD alimenta la propuesta y las especificaciones, pero **no se reemplaza por ellas**. Las especificaciones
describen comportamiento con detalle técnico y se fragmentan por capacidad. El PRD lo cuenta en lenguaje de
negocio, con un flujo coherente. Si una especificación contradice al PRD, manda el PRD: se cambia primero el
PRD y después la especificación.

La experiencia del autor encaja con esto: cuando el PRD llegaba bien cerrado, rara vez tenía que corregir lo
que producía el SDD.

### 7.5 Ventajas y costes

- **Ventajas:**
  - cada fase deja un artefacto, y eso da trazabilidad;
  - los puntos de revisión entre fases permiten frenar antes de escribir código;
  - el diseño técnico queda escrito y discutido;
  - encaja bien con cambios de alto riesgo o de mucha complejidad.
- **Costes:**
  - sobrecarga documental, con varios artefactos que hay que mantener coherentes;
  - más tiempo y más tokens;
  - el riesgo de aplicar la ceremonia completa a cambios pequeños que no la necesitan.

Las tres reglas de la sección 3 se aplican igual, con el prompt de la sección 5.3. El informe de cierre se
guarda junto a los artefactos del cambio.

---

## 8. El proceso ODD en detalle

### 8.1 El protocolo

ODD aplica estos siete pasos a cada pedido, sin que nadie se lo pida:

1. **Autorizar.** Primero establece si el pedido autoriza un cambio. Investigar, explicar o revisar es de solo
   lectura; implementar requiere un pedido explícito.
2. **Explorar** el código y los requisitos, en proporción al pedido.
3. **Resolver la incertidumbre.** Si queda una decisión de producto abierta, hace una pregunta concreta y
   espera.
4. **Clasificar el tamaño.** Una tarea es *pequeña* si se entiende, su riesgo está acotado y alguien podría
   terminarla solo con el pedido original y el `git diff`. Si no pasa esa prueba, es *grande*. El número de
   archivos o de líneas nunca decide el tamaño.
5. **Registrar el trabajo grande** en un documento de ejecución antes de la primera escritura.
6. **Implementar tarea por tarea**, con tests primero y un commit por tarea.
7. **Cerrar** con el resultado verificado, cada comprobación fallida o pendiente, la línea de riesgo y el
   informe de cierre.

El trabajo pequeño no genera ningún documento: se resuelve directo, con su test y la batería de pruebas.

La tabla de [Fases de ODD](fases.md) resume los siete pasos con los nombres que usa Gentle AI.

### 8.2 El documento de ejecución (*feature document*)

Para el trabajo grande, ODD crea `odd/tasks/<feature>.md` y una copia en la memoria persistente del agente,
para poder retomar si la sesión se corta. Tiene una estructura fija, con lo estable primero y lo que crece al
final:

| Sección | Contenido |
|---|---|
| Encabezado | Dos o tres líneas: objetivo, rama, estrategia de entrega, comando de tests, estimación de tiempo y hora real de inicio |
| `## Specs` | Requisitos numerados `S1…Sn` que **citan textualmente** el PRD: reglas, mensajes, ejemplos. Nunca resumen ni agregan requisitos que nadie pidió |
| `## Tasks` | Una línea por tarea: identificador, `S#` que cubre, ruta (directa o delegada), estado y commit |
| `## Log` | La bitácora. `L1` es el pedido original del usuario, textual; después se agregan correcciones, decisiones, evidencia y progreso |

El PRD sigue mandando: el encabezado lo dice expresamente ("*The PRDs win on any product question*"). Un
cambio de requisito agrega su entrada textual al log, reescribe solo el `S#` afectado y reabre solo su tarea.

Ejemplos reales:

- [crime-query.md](../../odd/tasks/crime-query.md) y [soporte-linux.md](../../odd/tasks/soporte-linux.md) usan
  esta estructura.
- [crime-generator.md](../../odd/tasks/crime-generator.md) es anterior y usa una versión previa de ODD, con
  tabla de decisiones y tabla de progreso. Sirve para ver cómo evolucionó el método.

### 8.3 El ciclo de cada tarea

1. **El agente lee el documento hasta el log** y sabe qué `S#` tiene que cubrir. Las tareas se pasan por
   referencia, por ejemplo "T2, S3–S4", nunca parafraseadas.
2. **Test primero: RED → GREEN.** Cuando hay un test determinista posible, se escribe primero y se ve fallar,
   después se implementa hasta que pase y por último se refactoriza con los tests en verde. Hay un test por
   cada regla pedida, y otro por cada comportamiento existente que el cambio toca.
3. **Comprobaciones de la tarea.** El test enfocado y la batería completa, más formato y lint:
   - El *test enfocado* es solo el test de la regla de la tarea, el que se vio fallar en RED. Se ejecuta solo,
     y responde en segundos si el cambio cumple la spec.
   - La *batería completa* ejecuta todos los tests, y comprueba que el cambio no rompió nada que ya funcionaba.
   - El *formato* comprueba que el código sigue el estilo estándar del lenguaje (sangría, espacios, saltos de
     línea). No cambia el comportamiento: solo evita diffs con ruido y discusiones de estilo.
   - El *lint* busca errores probables y código poco idiomático que compila pero conviene corregir.

   En Rust: `cargo test <filtro>` para el test enfocado, `cargo test --workspace` para la batería,
   `cargo fmt --all --check` para el formato y `cargo clippy` para el lint.
4. **Un commit por tarea**, en la rama del feature, con mensaje *Conventional Commits* y los tests y la
   documentación de ese comportamiento. El identificador del commit queda anotado en el documento como
   evidencia.
5. **Evaluación de riesgo del commit.** Un cambio es de alto riesgo si:
   - toca datos persistidos o efectos irreversibles;
   - toca seguridad;
   - cambia contratos que otros consumen;
   - toca concurrencia;
   - toca la entrega o el entorno;
   - o ningún test detectaría una regresión.

   El riesgo alto activa un **verificador independiente**: otro agente, que no escribió el código, prueba la
   especificación contra el resultado.
6. **Revisión automática.** Con el modo RDD de Gentle AI, cada commit de alto riesgo, o cada tramo de unas 400
   líneas, recibe una revisión con hasta cuatro lentes: riesgo, resiliencia, legibilidad y fiabilidad. No compara
   el código con la especificación, que es tarea del verificador: busca defectos que introdujo el cambio. Al
   cerrar se revisa también el último tramo, aunque no llegue a ese tamaño. Los
   hallazgos se convierten en tareas nuevas. En el generador así nacieron T2a, cuando un `--log` mal puesto
   podía borrar el CSV de origen, y T5a, cuando se podían borrar carpetas del usuario.
7. **Actualizar el documento y su copia** antes de pasar a la siguiente tarea.

Las correcciones tienen límite: una tanda que arregla todos los bloqueos reportados y una única recomprobación
de esos bloqueos. No hay ciclos de "revisar hasta que quede limpio".

### 8.4 Quién hace qué

Por defecto, el agente principal escribe directamente, siguiendo el documento. Solo delega en subagentes por
un motivo concreto:

- un **explorador** de solo lectura, cuando entender el código exige más evidencia de la que cabe en una
  consulta acotada;
- un **escritor** por unidad, cuando hay varias unidades independientes que conviene hacer en paralelo, o
  cuando el contexto de la sesión se está llenando;
- un **verificador** independiente, para los cambios de alto riesgo.

El tamaño de la tarea o el número de archivos, por sí solos, nunca justifican delegar.

### 8.5 El cierre

- Aceptación sobre datos reales cuando el PRD la define.
- **Línea de riesgo** de todo cambio de código: `Riesgo: ítem N (motivo)` o `Riesgo: ninguno`.
- **Informe de cierre** en el repositorio (sección 3).
- **Merge** a `main` después de la verificación completa. El push, los PR y el merge siguen siendo decisiones
  del usuario según la política del repositorio.

---

## 9. Bajo el capó: subagentes, memoria, skills y hooks

Desde fuera parece una sola conversación con la IA. Por dentro trabajan cuatro mecanismos de Claude Code y de
Gentle AI, que la sesión principal usa sin que el programador tenga que pedirlos uno a uno. Saber que existen
ayuda a entender qué está pasando y por qué el agente puede trabajar horas sin perder el hilo.

| Mecanismo | Qué es | Para qué sirve |
|---|---|---|
| **Subagentes** | Instancias de la IA con su propio contexto, lanzadas por la sesión principal | Repartir el trabajo sin llenar el contexto del orquestador; revisar con ojos que no escribieron el código |
| **Memoria persistente** | Información que sobrevive a la sesión: Engram, la memoria de Claude Code y `CLAUDE.md` | Retomar el trabajo, recordar decisiones y preferencias, no repetir errores |
| **Skills** | Instrucciones empaquetadas que se cargan solo cuando la tarea las necesita | Dar al agente un procedimiento probado (escribir un PRD, cerrar un cambio) sin cargarlo siempre |
| **Hooks** | Comandos que el entorno ejecuta automáticamente en ciertos momentos | Reglas que no dependen de que el agente se acuerde: bloquear, recordar, verificar |

### 9.1 Subagentes: la sesión principal orquesta

La sesión principal actúa como **orquestador**: entiende el pedido, decide y reparte. El trabajo pesado lo
hacen subagentes, cada uno con su propio contexto, que devuelven un resumen. Así el orquestador no se llena
de código leído y puede sostener una conversación larga.

En el experimento se usaron estos papeles:

- **Escritores.** En el generador, cada tarea (T1 a T7) la implementó un subagente escritor, con su lista
  exacta de archivos permitidos y los comandos de verificación que debía ejecutar. Cada uno devolvió un
  informe con la evidencia de RED, GREEN y los resultados.
- **Exploradores**, de solo lectura, para entender código que el orquestador no conoce.
- **Verificadores independientes**, que no escribieron el código y prueban la especificación contra el
  resultado.
- **Revisores**, uno por lente (riesgo, resiliencia, legibilidad, fiabilidad), que Gentle AI lanza en paralelo
  y en un entorno de solo lectura. Si una revisión detecta un bloqueo dudoso, un **refutador** intenta
  desmentirlo antes de exigir una corrección.

En el consultor, mientras un verificador comprobaba la T1, un escritor ya implementaba la T2. En SDD pasa lo
mismo: la sesión principal orquesta y cada fase la ejecuta un subagente.

La versión actual de ODD es más austera: el orquestador escribe directamente por defecto, y solo delega cuando
hay un motivo concreto (unidades independientes en paralelo, un contexto que se llena o un cambio de alto
riesgo que pide verificador).

Qué modelo y qué esfuerzo usa cada uno de estos papeles, cómo se configuran (en Gentle AI y en Claude Code) y
qué se observó en el experimento está en [Agentes de Gentle AI: modelos y esfuerzo](agentes-y-modelos.md). En
resumen: sin una definición propia, un subagente hereda el modelo y el esfuerzo de la sesión principal, y así
corrieron todos los subagentes del experimento, en Opus 5.5.

### 9.2 Memoria: tres capas

| Capa | Dónde vive | Qué guarda | Quién la ve |
|---|---|---|---|
| **`CLAUDE.md`** | En el repositorio, versionado | Las reglas del proyecto | Todo el que clona el repositorio |
| **Memoria de Claude Code** | En la máquina del usuario | Preferencias personales: "no me preguntes", "verifica siempre", "estima el tiempo" | Solo ese usuario |
| **Engram** | Un servidor MCP de memoria persistente | Decisiones, descubrimientos, errores corregidos, resúmenes de sesión y una copia de cada documento de ejecución | Las sesiones de ese usuario en ese proyecto |

Engram es la que más trabajo hace durante un cambio grande:

- **Guarda de forma proactiva** cada decisión y cada hallazgo, sin que se lo pidan. Por ejemplo, que el crate
  `extsort` perdía datos, o que el formato de tabla de `sstable` es el de LevelDB.
- **Guarda una copia del documento de ejecución** (`odd/<feature>/tasks`). Si la sesión se corta o el contexto
  se compacta, el agente retoma con `mem_context`, `mem_search` y `mem_get_observation`, y reconcilia con el
  archivo del repositorio antes de seguir.
- **Recoge lo que aprenden los subagentes.** Cada subagente termina su informe con un bloque de aprendizajes
  clave, que Engram captura sin intervención.
- **Cierra cada sesión con un resumen** del objetivo, los descubrimientos, lo hecho y los próximos pasos.

Además, Claude Code guarda el historial de cada sesión en la máquina. Este documento lo aprovechó: la
conversación del PRD del consultor se reconstruyó a partir de ese historial.

### 9.3 Skills: procedimientos que se cargan cuando hacen falta

Una skill es un `SKILL.md` con reglas, pasos y, a veces, plantillas o scripts. Su descripción dice cuándo
aplica, y el agente la carga solo en ese momento. Esa es la diferencia con `CLAUDE.md`, que se carga siempre
y por eso tiene que ser corto.

| Skill | Origen | Para qué |
|---|---|---|
| `prd` | Propia del proyecto | Escribir el PRD con sus reglas y su plantilla |
| `closing-report` | Propia del proyecto | Escribir el informe de cierre, con plantilla y un script con tests |
| `vibe-coding` | Propia del proyecto | Fijar los bordes del vibe coding: mapa del código existente, preguntas y estimación antes; DRY, tests por criterio, verificación e informe después |
| `worktree` | Propia, de otro proyecto | Crear, fusionar y eliminar worktrees (sección 10) |
| `work-unit-commits`, `chained-pr` | Gentle AI | Partir el trabajo en commits y PRs revisables |
| `judgment-day`, `cognitive-doc-design` | Gentle AI | Revisión adversarial doble; documentos fáciles de leer |
| `skill-registry` | Gentle AI | Mantener el índice de skills |

El **registro de skills** (`.atl/skill-registry.md`) es un índice que el orquestador consulta antes de delegar.
Cuando lanza un subagente, le pasa la ruta exacta de las skills que aplican a su tarea, para que las lea antes
de empezar.

### 9.4 Hooks: reglas que no dependen de la memoria del agente

Un hook es un comando que Claude Code ejecuta automáticamente en un momento dado: antes de cada orden de
shell, al empezar una sesión, al terminar un turno. En el experimento aparecieron tres:

- **Al empezar la sesión**, Engram inyecta el contexto de las sesiones anteriores.
- **Al terminar un turno**, Gentle AI comprueba si quedan cambios sin revisar y, si los hay, impide dar el
  trabajo por terminado hasta pasar la revisión.
- **Antes de cada orden de shell**, en el proyecto de los worktrees, una guarda bloquea cualquier comando que
  apunte a la base de datos principal (sección 10.4).

La idea de fondo es la misma que con los scripts: lo que es crítico no se deja a la buena memoria del agente,
se automatiza.

---

## 10. Desarrollo en paralelo con worktrees

Con un PRD cerrado y las tres reglas de la sección 3, un agente puede trabajar horas sin interrumpir. Eso
habilita el paso siguiente: **varios cambios a la vez**, cada uno en su propia copia del repositorio y en su
propia sesión de agente, mientras el desarrollador avanza con otro. El autor ya trabaja así, y en el
experimento las revisiones automáticas corrían en worktrees limpios, aparte del árbol de trabajo.

La guía completa, aplicable a cualquier proyecto con base de datos, es el anexo
[Desarrollo en paralelo con worktrees](parallel-developing.md). Esta sección resume lo esencial.

### 10.1 El problema

`git worktree` aísla **el código**: cada carpeta tiene su rama y sus archivos. Pero casi todo lo demás sigue
compartido, y ahí se rompe el trabajo en paralelo:

| Recurso | Qué pasa si se comparte |
|---|---|
| **Base de datos** | La rama A publica su esquema y rompe la rama B; las pruebas de una borran los datos de la otra |
| Configuración local y secretos | Las dos copias apuntan a la misma base sin que nadie lo note |
| Dependencias | La copia nueva no compila hasta instalarlas |
| Puertos | La segunda app no arranca, o el navegador habla con la otra copia |
| Instrucciones privadas del agente | La copia nueva trabaja sin las reglas del desarrollador |

La regla de diseño es una sola: **cada copia es un entorno completo y autónomo**. Lo que no se pueda aislar se
declara como riesgo aceptado, con una regla de uso que lo neutraliza.

### 10.2 El modelo: un nombre, todo derivado

El desarrollador dice un solo nombre, el del cambio (por ejemplo `orders-export`), y de él sale todo:

- la rama `feature/orders-export`;
- la carpeta `<raíz-wt>/<proyecto>-orders-export`, siempre fuera del repositorio principal;
- la base `<Proyecto>_wt_orders_export`. El prefijo fijo permite que los scripts destructivos se nieguen a tocar
  cualquier otra base;
- un archivo de configuración local, ignorado por git, que dice qué base usa esa copia.

**La regla de la base:** la base de una copia es la que dice su archivo local. En un worktree ese archivo es
obligatorio. Si falta, todo falla nombrando el archivo. Nunca se recurre en silencio a la base principal. Y
nadie escribe el nombre de la base: ni el desarrollador ni el agente. Un script lo resuelve.

### 10.3 El ciclo de vida

| Operación | Desde dónde | Regla clave |
|---|---|---|
| Crear | Copia principal | Validar todo antes de tocar nada; si un paso falla, decir qué quedó creado |
| Trabajar | El worktree | Abrir el agente **en la raíz** del worktree, porque si no, no carga sus hooks |
| Fusionar | El worktree | Primero la integración entra en la rama y se prueba; solo después la rama entra en la integración. Los archivos generados se regeneran; los conflictos escritos a mano se resuelven con el desarrollador |
| Eliminar | Copia principal | Si se pierde algo (cambios sin commit, commits sin integrar), se rechaza. Nunca se fuerzan ni se matan procesos |

### 10.4 Scripts, skill y guarda

- **La lógica vive en scripts versionados**, no en el prompt. Un script se prueba, se revisa y funciona igual
  con o sin IA. Los scripts no hacen preguntas, se niegan cuando algo no cuadra, dan mensajes que dicen qué
  hacer y usan códigos de salida con significado. Por ejemplo, `3` en una fusión significa "reintentar una
  sola vez".
- **La skill es una tabla de enrutado** que asocia cada frase del equipo con un script, más las reglas duras.
  No contiene lógica.
- **La guarda es un hook** que se ejecuta antes de cada orden de shell. Dentro de un worktree, bloquea
  cualquier orden que nombre la base principal y sugiere la alternativa exacta. Es una red contra la costumbre,
  no contra un adversario.

### 10.5 Prompts de cada operación

| Desde | Prompt |
|---|---|
| La copia principal | «Crea un worktree para el cambio `<n>`.» · «¿Qué worktrees tengo?» · «Elimina el worktree `<n>`.» |
| El worktree | «Verifica este worktree: qué base resuelve, si existe y si `git status` está limpio.» |
| El worktree | «Empecemos el cambio `<n>`.» Aquí arranca el flujo de siempre: PRD y después vibe coding, SDD u ODD |
| El worktree | «Fusiona el cambio a `<integración>`.» |

---

## 11. Evidencia del experimento

| Cambio | PRD | Ejecución | Resultado |
|---|---|---|---|
| Generador | [generador.prd.md](../prd/chicago-crime/generador.prd.md) | [crime-generator.md](../../odd/tasks/crime-generator.md) | 230 tests; aceptación 14/14 sobre 8.656.904 registros, generados en 90,9 s (unos 95.000 por segundo, según el [resumen de la ejecución](contexto_prd_vibe_odd_sdd.md)); pico de memoria de ~93 MiB frente a un límite de 512 MiB; las dos generaciones salieron idénticas byte a byte |
| Consultor | [consultor.prd.md](../prd/chicago-crime/consultor.prd.md) | [crime-query.md](../../odd/tasks/crime-query.md) | 423 tests; aceptación 58/58; consulta por ID en menos de 1 ms, el tiempo que muestra el propio consultor (39–51 ms con el arranque del proceso, medidos en la aceptación); ~3 h 15 min frente a 4–7 h estimadas |
| Soporte Linux | [soporte-linux.prd.md](../prd/chicago-crime/soporte-linux.prd.md) | [soporte-linux.md](../../odd/tasks/soporte-linux.md) | ~48 min frente a 1,5–2,5 h estimadas |

Lo que muestra el caso:

- **Las revisiones automáticas encontraron defectos reales que los tests no cubrían.** Por ejemplo, el crate
  `extsort` perdía datos, el `seek` de `sstable` tenía un bug y los bloques corruptos se saltaban en silencio.
- **El usuario corrigió a la IA, y la IA corrigió al usuario, siempre con evidencia.** Se ve en
  [Cómo se generó el PRD](generacion-del-prd.md).

---

## 12. Qué sabemos y qué no

**Observaciones que el experimento respalda:**

1. Un PRD bien trabajado da un contrato funcional lo bastante preciso para que ODD genere un plan de
   implementación detallado.
2. El documento de ejecución separa las decisiones de producto, que son del PRD, de las técnicas, que son
   suyas.
3. El agente produjo una implementación en Rust que no es trivial, aunque el autor no conocía el lenguaje.
4. Hay cifras reales de rendimiento, de memoria y de tamaño, junto con una batería amplia de pruebas.
5. Tener artefactos persistentes hace el trabajo auditable y compartible.

**Hipótesis que faltan medir:**

- que PRD + ODD requiere menos intervención humana que PRD + SDD;
- que PRD + ODD cuesta menos tiempo o tokens que PRD + SDD con la misma calidad;
- que PRD + vibe coding da resultados comparables en cambios equivalentes;
- que la calidad del PRD alcanza por sí sola para una autonomía alta. También pesan la arquitectura, los
  agentes, las herramientas y las verificaciones.

**Métricas recomendadas para una comparación real:**

- horas humanas dedicadas al PRD;
- el modelo y el esfuerzo usados en cada fase;
- el tiempo y el coste del agente;
- las intervenciones humanas durante la implementación;
- cuántos cambios fueron de requisitos y cuántas correcciones técnicas;
- los defectos que encontraron los revisores;
- repetir un cambio comparable con cada enfoque.

---

## 13. Referencias

- Repositorio del experimento: <https://github.com/jesuslpm/chicago-crime>
- [Cómo se generó el PRD](generacion-del-prd.md): ejemplo paso a paso.
- [Agentes de Gentle AI: modelos y esfuerzo](agentes-y-modelos.md): papeles, configuración y lo observado.
- [Desarrollo en paralelo con worktrees](parallel-developing.md): guía completa, anexo de la sección 10.
- [Contexto: PRD + vibe coding, ODD y SDD](contexto_prd_vibe_odd_sdd.md): resumen de la conversación con
  ChatGPT que originó este documento.
- Skill PRD y plantilla: [`.claude/skills/prd`](../../.claude/skills/prd/SKILL.md) ·
  [Gist](https://gist.github.com/jesuslpm/fc36d3503116171eee409b313365cde7)
- Skill de informe de cierre: [`.claude/skills/closing-report`](../../.claude/skills/closing-report/SKILL.md)
- Instrucciones del proyecto: [`CLAUDE.md`](../../CLAUDE.md)
- Solicitud de SDD opcional en Gentle AI: <https://github.com/Gentleman-Programming/gentle-ai/issues/5475>
- Dataset original: <https://data.cityofchicago.org/Public-Safety/Crimes-2001-to-Present/ijzp-q8t2>
