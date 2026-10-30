# Comprobación de organización recordada de los nodos — PRD

**Versión**: 1.0
**Fecha**: 2026-10-30
**Documento padre**: [Árboles B+ de Krummi — PRD](trees.prd.md) (v1.0)

## Objetivo y contexto

Hacer más baratas las lecturas de los árboles sin dejar de comprobar la integridad de cada página que se lee del
archivo de datos. Hoy, cada vez que una operación lee un **nodo** (una página de un árbol normal, del catálogo, de un
árbol multivalor o del subárbol de una de sus claves), el motor hace dos comprobaciones de la página entera:

- la **comprobación de integridad** (G6), un código de 64 bits que detecta los bytes alterados, solo en las páginas
  que vienen del archivo de datos o, en modo temporal, de la memoria del almacén;
- la **comprobación de organización** del nodo, que confirma que su disposición interna (tipo, número de entradas,
  posición y tamaño de cada una, sin huecos ni solapes) es coherente antes de usarlo, venga de donde venga.

La segunda recorre todas las entradas del nodo, así que cuesta más cuanto más entradas tiene, y se repite en cada
lectura aunque el nodo no haya cambiado. Además de esas dos, una lectura hace otras comprobaciones que este cambio no
toca: que la página pueda contener datos, el nivel que se espera del nodo y la coherencia de cada entrada que usa (el
contenido de una entrada de un árbol multivalor, el de una entrada del catálogo, las referencias a valores guardados
fuera de su hoja).

Este cambio hace que el motor **recuerde los nodos cuya organización ya comprobó** mientras el almacén está abierto,
identificados por su número de página y por el código de integridad que esa misma lectura acaba de verificar. La
comprobación de integridad se sigue haciendo en cada lectura, como hoy.

**Por qué**: en las hojas densas, como las de los subárboles de un árbol multivalor con valores de 16 bytes (unas 370
entradas), la comprobación de organización cuesta varias veces más que la de integridad. En la investigación que
siguió al cierre de la 5.4, Contiene en un subárbol pasó de 6,4–7,0 µs a 2,8–3,1 µs sin la comprobación de
organización y manteniendo la de integridad; en un árbol normal con claves compuestas, de 3,7–4,1 µs a 3,3–3,4 µs.

**Por qué ahora**: los árboles multivalor (5.4) son los primeros que llenan hojas con cientos de entradas pequeñas, y
las tablas con índices secundarios (módulo 7) se apoyarán en ellos.

Este PRD es un cambio del módulo de árboles fuera de su reparto en seis PRD hijos (B2): no es uno de ellos ni cuenta
para el cierre del módulo. Hereda todo lo del padre y sus ascendientes; aquí solo se añade cuándo se repite la
comprobación de organización. Lo que cambia reglas anteriores está declarado en sus decisiones y anotado en los
documentos afectados (ver [Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

### Decisiones de producto

| ID | Decisión |
|---|---|
| C1 | **Qué se recuerda**: que el contenido de un nodo pasó la comprobación de organización, identificado por el número de su página y por su código de integridad. Solo se recuerdan comprobaciones superadas: un nodo que no la pasa nunca se recuerda. Las demás comprobaciones de la lectura (que la página pueda contener datos, el nivel del nodo, la coherencia de cada entrada que se usa) no se recuerdan y se hacen como hoy, en cada uso. |
| C2 | **Orden en cada lectura de un nodo**: primero se comprueba la integridad de los bytes que se leen; si falla, la operación falla con la clase de daño, como hoy (G6). Si pasa y el motor recuerda ese número de página con ese mismo código, se acepta la organización del nodo sin comprobarla. Si no lo recuerda, se comprueba: si falla, la operación falla con la clase de daño y el número de la página, como hoy; si pasa, desde ese momento se recuerda. Nunca se usa el código de integridad de una lectura anterior. |
| C3 | **Qué lecturas lo usan**: solo las lecturas de nodos cuya integridad se comprueba en esa misma lectura, es decir, las que vienen del archivo de datos o, en modo temporal, de la memoria del almacén. No lo usan, y comprueban la organización en cada lectura como hoy: las versiones confirmadas que todavía están en el área temporal (J7, T16) y las páginas que modifica la propia escritura, que hoy no pasan la comprobación de integridad. Las páginas que no son nodos (las del mapa de espacio libre, F9; el encabezado; las de los valores guardados fuera de su hoja) no tienen comprobación de organización y no cambian. Leer del archivo de datos un contenido que antes se leyó del área temporal no lo encuentra recordado: se comprueba y se recuerda. |
| C4 | **Alcance y vida**: hay una sola memoria de nodos recordados por almacén abierto, también en modo temporal, compartida por todas sus transacciones, árboles e hilos. Empieza vacía al abrir el almacén, se descarta al cerrarlo y no se guarda en disco: el formato no cambia (B12) y un almacén se abre igual con o sin este cambio. |
| C5 | **Memoria acotada**: lo recordado ocupa un tamaño fijo que no depende del tamaño del almacén ni del número de lecturas. Cuando no hay sitio, un nodo recordado puede dejar de estarlo; solo cuesta volver a comprobar su organización. Cada recuerdo se guarda y se consulta entero: una consulta nunca ve un recuerdo a medio guardar ni mezcla partes de dos. Una consulta encuentra un nodo recordado cuando su código coincide con el recordado para el número de su página; como el código cubre el número de página, que coincida con el de otra página exige la misma coincidencia de 64 bits que C7. El tamaño es interno: no es una opción del almacén, no aparece en su información (P12) y no hace fallar ninguna operación. |
| C6 | **Lecturas concurrentes**: consultar y añadir recuerdos no añade esperas bloqueantes entre lecturas ni con la escritura. Varias lecturas que leen a la vez un nodo no recordado pueden comprobar su organización cada una; el resultado es el mismo. |
| C7 | **Garantía resultante** (excepción a G6 del padre raíz, anotada en él): toda página que se lee del archivo de datos sigue pasando la comprobación de integridad en cada lectura, así que todo lo que esa comprobación detecta hoy se sigue detectando. Lo que cambia es la segunda defensa. Hoy, si los bytes de un nodo cambian sin que el motor los escriba (un fallo del hardware o de la memoria, o un programa ajeno, que P16 deja fuera de alcance) y la comprobación de integridad no lo detecta por una coincidencia de su código de 64 bits, la comprobación de organización todavía puede detectarlo. Con este cambio, si ese número de página con ese código ya se recordó en la apertura en curso, el nodo se acepta sin comprobar su organización. Así, una alteración que coincide en el código ya no necesita además dejar una organización coherente para pasar inadvertida. En ese caso, la operación puede devolver datos que no son los escritos o terminar con un pánico en lugar de un error de daño (y no con el error de G12). Usar un nodo mal organizado no provoca por sí mismo un comportamiento indefinido, porque los árboles no usan código no seguro (D12); la excepción de P16 sigue igual: si un programa ajeno modifica los archivos con el almacén abierto, el comportamiento puede ser indefinido, como hoy. Se acepta como riesgo residual. Un almacén que se abre empieza sin nada recordado, así que las alteraciones hechas con el almacén cerrado se detectan como hoy (AC10). |
| C8 | **Coherencia de las páginas**: se apoya en las reglas vigentes del traslado (J5, T14): el motor solo reescribe una página en su sitio del archivo de datos cuando ninguna transacción de lectura abierta necesita la versión anterior, así que los bytes de un nodo que una lectura comprueba no cambian mientras los comprueba ni mientras la recuerda en uso. |
| C9 | **Medición** (B12, sin umbral por D5): el cambio incluye una medición reproducible que se ejecuta a mano, en Windows y en Linux, y que no forma parte de la integración continua ni de las pruebas normales. Mide, sin recordar nodos y recordándolos, el coste de leer nodos hoja de 1, 100 y 370 entradas, y Contiene en un árbol multivalor con los valores en subárboles y en un árbol normal con claves compuestas, en almacenes en disco. El informe de cierre incluye sus resultados, también si algún caso es más lento con el cambio. |

### Estado de partida

Verificado el 2026-10-30 en el código:

| Aspecto | Situación |
|---|---|
| Comprobación de integridad | Código de 64 bits sobre toda la página, verificado en cada lectura de una página del archivo de datos o de la memoria del almacén temporal. No se verifica en las versiones del área temporal ni en las páginas que modifica la escritura. |
| Comprobación de organización | Se hace en cada lectura de un nodo, venga de donde venga, y recorre todas sus entradas. Junto a ella se comprueban que la página pueda contener datos y el nivel esperado; la coherencia de cada entrada se comprueba al usarla. |
| Versiones de página | La cabecera de una página guarda su código de integridad y su número; no lleva versión. Las versiones confirmadas viven en el área temporal hasta que el traslado las escribe en su sitio (J5, T14). Las páginas liberadas se reutilizan con otro contenido, y al crecer el archivo una página puede cambiar de dirección en memoria. |
| Memoria de comprobaciones | No existe: ninguna comprobación se recuerda entre lecturas. |
| Pruebas de daño | Las que alteran los archivos lo hacen con el almacén cerrado y lo reabren. Una prueba de los árboles confirma, a través de una escritura, un nodo con la organización dañada y la integridad correcta y espera el daño en dos lecturas seguidas; esa página se lee del área temporal. |
| Código no seguro | Los árboles no usan código no seguro (D12): un nodo mal organizado que se usara sin comprobar provocaría un pánico o resultados erróneos, no un acceso fuera de la memoria. |
| Precedente | La 5.3 dejó a la implementación si la integridad de un valor fuera de su hoja se comprueba en cada lectura o una vez por transacción, siempre que se cumpla G6. |

Una medición aislada del 2026-10-30 (Windows, compilación optimizada, en una copia temporal que no se conserva;
orientativa, la sustituye la de C9) dio para la comprobación de integridad unos 0,3 µs por página en caché y 0,5–0,6
µs fuera de ella, sea cual sea su contenido, y para la de organización unos 0,03 µs con 1 entrada, 0,1 µs con 10,
0,8–1,0 µs con 100, 1,4–2,0 µs con 200 y 2,5–2,9 µs con 370: entre 4 y 10 veces la de integridad en las hojas más
densas, y menos que ella en los nodos con pocas entradas.

## Actores

- **Desarrollador integrador**: lee y escribe árboles como hasta ahora; no cambia nada de lo que usa.
- **Mantenedor**: ejecuta en Windows y en Linux las pruebas y la medición de C9.

## Flujo

1. La aplicación abre un almacén; el motor empieza sin nodos recordados (C4).
2. Una operación lee un nodo del archivo de datos: el motor comprueba su integridad y, si no recuerda ese nodo con
   ese código, su organización, y lo recuerda si la pasa (C2).
3. Las lecturas siguientes de ese mismo contenido, desde cualquier transacción, comprueban su integridad y no repiten
   la de organización mientras siga recordado (C4, C5).
4. Si una escritura cambia el nodo, su nueva versión se lee del área temporal, con la comprobación de organización en
   cada lectura (C3); cuando el traslado la escribe en el archivo de datos, su código es otro y la primera lectura que
   la ve de ahí la comprueba (C1, C2).
5. Al cerrar el almacén, lo recordado se descarta.

## Validaciones y mensajes

No hay situaciones, clases de error ni datos de error nuevos. Un nodo cuya integridad u organización falla sigue
fallando con la clase de daño y el número de su página (D8), en cada operación que lo necesite (A9, M11).

## Salida

Nada de lo que devuelve una operación cambia de forma: los mismos datos, las mismas clases de error y los mismos datos
de error. La información del almacén (P12) no añade datos y la versión de formato sigue siendo `8`. Por ejemplo, la
segunda lectura de un nodo con la organización dañada falla igual que la primera:

| Dato del error | Primera lectura | Segunda lectura |
|---|---|---|
| Clase | daño | daño |
| Carpeta | la del almacén | la del almacén |
| Página | `<número>` | `<número>` |

## Reutilización

- La comprobación de integridad de las páginas y la clase de daño (G6), sin cambios.
- La comprobación de organización de los nodos, sin cambios en lo que comprueba.
- Las reglas del traslado (J5, T14), de las que depende C8.
- La medición de los árboles multivalor (M15), como base de la de C9.

## Fuera de alcance

- Dejar de comprobar la integridad en cada lectura, o recordarla (C7).
- Comprobar la integridad de las versiones del área temporal y de las páginas que modifica la escritura, y recordar
  su organización (C3).
- Recordar las demás comprobaciones de una lectura: el nivel del nodo y la coherencia de las entradas (C1).
- Las páginas que no son nodos: mapa de espacio libre, encabezado y valores fuera de su hoja (C3).
- Detectar que otro programa modifica los archivos con el almacén abierto (P16).
- Códigos de integridad más fuertes o criptográficos.
- Abaratar la propia comprobación de organización, o comprobar solo las entradas que usa la operación.
- Recordar nodos ya decodificados o caminos completos.
- Guardar lo recordado entre aperturas.
- Una opción pública para el tamaño o para desactivarlo, y estadísticas o contadores de aciertos en la información
  del almacén (C5).
- Los árboles de tamaño fijo (5.5), la compresión de páginas hoja (5.6) y el cifrado (módulo 9): cada uno decide si
  usa lo recordado y sobre qué bytes se calcula su código.
- Umbrales de rendimiento (módulo 6, D5).

## Dejado a la implementación

- El tamaño de la memoria de nodos recordados, como constante interna, cómo se reparte entre los números de página,
  qué recuerdo se descarta cuando no hay sitio y cuándo se reserva esa memoria (C5). Las pruebas pueden usar un
  tamaño propio.
- Si se usa para todos los nodos o solo a partir de cierto número de entradas, donde la medición de C9 muestre que
  compensa.
- Cómo sabe la lectura de un nodo que su integridad se comprobó en esa misma lectura (C2, C3).
- Cómo la medición compara sin recordar y recordando sin una opción pública (C5, C9), y las cargas exactas, unidades
  y repeticiones de la medición, dentro de C9.

## Criterios de aceptación

Valen los de los ascendientes, que deben seguir pasando sin cambios, incluidos AC10 del padre raíz, AC-M15 y el
resto de pruebas de daño de los árboles. Cada criterio se comprueba en un almacén en disco y en modo temporal, salvo
las partes que reabren el almacén, que son solo en disco. Las pruebas usan el conocimiento de la implementación para
contar las comprobaciones de organización, para colocar páginas en el archivo de datos o en el área temporal, para
alterar bytes y para elegir el tamaño de la memoria de nodos recordados.

1. **AC-C1** — Dados un árbol normal, el catálogo y el subárbol de una clave de un árbol multivalor, con sus páginas
   en el archivo de datos y sitio para recordar todos los nodos que lee la prueba, cuando una transacción de lectura
   lee la misma clave o el mismo valor varias veces y otra transacción de lectura lo lee después, entonces todas
   devuelven lo mismo, la organización de cada nodo del camino se comprueba solo en la primera lectura de ese nodo y
   su integridad en todas (C1, C2, C4).
2. **AC-C2** — Dado un nodo recordado, cuando una escritura cambia su contenido y se confirma mientras una transacción
   de lectura abierta antes sigue leyéndolo, entonces esa transacción sigue viendo el contenido anterior; y cuando
   esa transacción se cierra y la nueva versión se traslada al archivo de datos en el mismo número de página,
   entonces una transacción de lectura nueva ve el contenido nuevo y comprueba su organización una vez (C1, C2, C8,
   G3).
3. **AC-C3** — Dada una página de un nodo recordado que se libera y una escritura posterior reutiliza para otro nodo
   con otro contenido, cuando ese nodo nuevo llega al archivo de datos y se lee, entonces se lee su contenido y su
   organización se comprueba una vez (C1, C2).
4. **AC-C4** — Dada una página de un nodo en el archivo de datos con la organización dañada y el código de integridad
   recalculado para que sea correcto, cuando se lee dos veces en la misma transacción de lectura y una vez en otra,
   entonces cada lectura falla con la clase de daño y el número de esa página, y una lectura posterior la comprueba
   otra vez; también cuando antes se había recordado otra versión correcta de esa misma página (C1, C2, A9).
5. **AC-C5** — Dada la misma página dañada de AC-C4, pero en el área temporal (confirmada y sin trasladar), cuando se
   lee dos veces, entonces falla igual las dos veces (C3).
6. **AC-C6** — Dada una hoja de un árbol multivalor en el archivo de datos con la organización correcta y recordada,
   y una entrada cuyo contenido es incoherente, cuando una operación la usa dos veces, entonces falla las dos veces
   con la clase de daño y el número de esa página (C1, M11).
7. **AC-C7** — Dado un nodo recordado, cuando la prueba altera un byte de su página en el archivo de datos sin
   recalcular su código de integridad, entonces la lectura siguiente falla con la clase de daño y el número de esa
   página (C2, C7).
8. **AC-C8** — Dada una memoria de nodos recordados de un tamaño reducido para la prueba, y nodos correctos y uno
   dañado como en AC-C4 cuyos números de página la obligan a descartar recuerdos, cuando cuatro hilos con
   transacciones de lectura los leen a la vez, al menos mil veces cada uno, mientras una escritura los cambia y se
   trasladan, entonces cada lectura devuelve los datos de su instantánea y el nodo dañado falla en todas sus
   lecturas con la clase de daño (C5, C6).
9. **AC-C9** — Dado un almacén en disco con nodos recordados, cuando se cierra y se vuelve a abrir, entonces la
   primera lectura de cada nodo comprueba su organización otra vez (C4).
10. **AC-C10** — Dada la medición de C9, cuando se ejecuta en Windows y en Linux, entonces informa el coste por
    lectura de nodos hoja de 1, 100 y 370 entradas y el de Contiene en las dos cargas, sin recordar nodos y
    recordándolos, y los resultados quedan en el informe de cierre (C9).

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes:

- Padre raíz, G6 y AC10: excepción de C7. La comprobación de organización de un nodo cuya integridad se acaba de
  verificar se omite si ese mismo contenido (número de página y código de integridad) ya la pasó en la apertura en
  curso; una alteración que coincide en el código de integridad de un contenido recordado puede devolver datos que no
  son los escritos o terminar en un pánico. Las alteraciones hechas con el almacén cerrado se detectan como hoy.
- Árboles B+, B2: este cambio no es uno de los seis PRD hijos del reparto ni cuenta para el cierre del módulo.
- Árboles y operaciones por clave, A9, y árboles multivalor, M11: un nodo dañado sigue fallando con daño en cada
  operación que lo necesite, salvo el riesgo residual de C7.

## Referencias

- Padre: [trees.prd.md](trees.prd.md) v1.0
- Raíz: [krummi.prd.md](krummi.prd.md) v1.1
- Relacionados: [pager.prd.md](pager.prd.md) (P12, P15–P17), [transactions.prd.md](transactions.prd.md) (T14, T16),
  [journal.prd.md](journal.prd.md) (J5, J7), [free-space.prd.md](free-space.prd.md) (F9),
  [trees-keys.prd.md](trees-keys.prd.md) (A9), [trees-large-values.prd.md](trees-large-values.prd.md) (V7 y su
  *Dejado a la implementación*), [trees-multivalue.prd.md](trees-multivalue.prd.md) (M11, M15)
- Investigación previa: registro de la ejecución de los árboles multivalor, entrada L22
