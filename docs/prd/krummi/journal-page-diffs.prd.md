# Diario de los cambios de cada página — PRD

**Versión**: 1.0
**Fecha**: 2026-11-08
**Documento padre**: [Diario y recuperación de Krummi — PRD](journal.prd.md) (v1.0)

## Objetivo y contexto

Hacer más baratas las confirmaciones guardando en el diario solo **los bytes que cambian** en cada página, en lugar
de la página entera. Hoy, cada confirmación escribe en el diario una copia completa de cada página que modificó,
comprimida cuando se comprime (J16). Guardar una entrada de 100 bytes en una hoja cambia unos pocos cientos de bytes
de una página de 8 KiB, pero el diario recibe, comprime y lleva al disco la página entera, y lo mismo cada rama del
camino que cambia.

Con este cambio, de cada página que la confirmación modificó a partir de una versión anterior el diario guarda el
valor nuevo de los bytes que cambiaron, y de cada página que la confirmación creó, la página entera sin sus partes
que son ceros. La recuperación aplica eso en orden y llega a las mismas páginas que hoy. Para que borrar una entrada o
cambiar su tamaño cambie pocos bytes, el motor deja de reorganizar el nodo en cada borrado: el sitio que deja la
entrada queda como un **hueco** hasta que hace falta para una entrada nueva.

**Por qué**: en el banco comparativo (módulo 6), Krummi incumple el umbral de las cargas de escritura al azar:
`carga-al-azar` (W2) 0,40 en Windows y 0,48 en Linux, `borrado` (W14) 0,47 y 0,55, `reutilizacion` (W15) 0,42 y
0,52 y `multivalor-carga` (W19) 0,37 y 0,55 (informes `2026-11-07-*-write-reuse` de `docs/benchmark`). Según O8 del
plan de rendimiento, la compresión del diario es el 36–40 % de W2, W14, W15 y W19 y el 23 % de W21. Los perfiles en
Linux del 2026-11-08 ponen el 61 % de `tamano-fijo-carga` (W21, 0,65 y 0,71) en la confirmación (copiar, comprimir y
calcular el código de las páginas enteras); W21 gana solo con Y1, porque sus nodos no cambian (Y4). Voron guarda en su
diario las diferencias de cada página con su versión anterior.

**Por qué ahora**: es O8 del plan de rendimiento, la mayor palanca que queda para las escrituras al azar, y la
primera de la investigación del 2026-11-08 sobre lo que separa a Krummi de Voron. Invierte el punto de Fuera de
alcance de la escritura agrupada que dejaba para después «guardar en el diario solo las diferencias de cada página».

Este PRD es un cambio del diario y de la organización de los nodos. Hereda todo lo del padre y sus ascendientes; aquí
solo se añade qué guarda el diario de cada página, cómo lo aplica la recuperación y cuándo se reorganiza un nodo. Lo
que cambia reglas anteriores está declarado en sus decisiones y anotado en los documentos afectados (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

### Decisiones de producto

| ID | Decisión |
|---|---|
| Y1 | **Qué guarda el diario de cada página** (precisa J16). De una página que la confirmación obtuvo de su última versión confirmada para modificarla, el diario guarda el valor nuevo de cada byte que difiere de esa versión, nunca su cambio respecto del valor anterior. Una página así que no cambió ningún byte no se guarda. De una página que la confirmación creó (una página que reserva, nueva o liberada antes, y cada página de un valor fuera de su hoja, también cuando reutiliza páginas), el diario guarda siempre la página entera, sin escribir sus partes que son ceros; una página creada toda de ceros también consta. Lo que se guarda de una página nunca ocupa más que la página entera. Después se comprime como hoy cuando ocupa menos comprimido. |
| Y2 | **Recuperación** (cambia J8). La apertura aplica a cada página de `krummi.data`, en el orden en que se confirmaron, lo que el diario guarda de ella desde lo que consta como trasladado: una página creada se escribe entera, con ceros donde no se guardó nada; de las demás, se escriben los bytes guardados sobre el contenido que tiene la página en `krummi.data`, o sobre ceros si la página está más allá de su final. Como cada byte guardado es su valor final en esa confirmación, el resultado es la última versión confirmada de cada página aunque una caída haya cortado un traslado y dejado en `krummi.data` páginas con bytes de varias versiones posteriores a lo trasladado, aunque se corte una recuperación anterior y aunque la recuperación empiece desde la copia más antigua del encabezado (J8). Al terminar de aplicar, la apertura comprueba la integridad (G6) de cada página que aplicó cuya última aplicación no fue una página de un valor fuera de su hoja, que no la tiene por página. Lo que J9 llama daño del diario se sigue detectando antes de tocar el almacén, como hoy. |
| Y3 | **Página dañada al recuperar** (cambia J8 y G5 en esto). Si una página aplicada no pasa la comprobación de Y2, la apertura falla con la clase de daño y el número de la menor de esas páginas, después de haber aplicado todo lo demás: el almacén queda modificado, y `krummi.data` puede haber crecido, como en una recuperación cortada. La siguiente apertura repite la recuperación y falla igual. Puede pasar cuando su contenido en `krummi.data` estaba dañado por algo que no es un traslado cortado (un fallo del disco o un programa ajeno, P16), o cuando J9 descarta por dañada una confirmación que ya se había trasladado en parte. Hoy, en esos casos, las páginas que el diario guarda se reescriben enteras sin error. Se acepta: la recuperación no puede distinguir esos daños de un fallo del motor, y no los oculta. Si antes falla otra cosa (falta de espacio o de memoria, un fallo de entrada y salida), la apertura falla con esa clase como hoy (J8), y la siguiente repite. |
| Y4 | **Huecos en los nodos**. En todos los nodos de los árboles con entradas de tamaño variable (normales, el catálogo, los árboles multivalor y los subárboles de sus claves), borrar una entrada o sustituirla por otra de distinto tamaño deja su sitio como un hueco, sin mover las demás entradas, también en modo temporal; sustituir una entrada por otra del mismo tamaño la cambia en su sitio, como hoy. Los bytes de un hueco no se borran, como hoy no se borran los del espacio libre. Un nodo sin entradas no tiene huecos. Un nodo se reorganiza para usar sus huecos solo cuando una entrada nueva no cabe sin ellos y sí cabe con ellos. Lo que ocupa un nodo para dividirse, fusionarse o considerarse poco lleno es lo que ocupan sus entradas, como hoy, sin contar sus huecos: ningún árbol tiene más ni menos páginas que hoy tras las mismas operaciones. La comprobación de organización de un nodo acepta huecos y sigue rechazando entradas que se solapan, que se salen de la página o que superan sus tamaños máximos, y lo que el nodo diga de sus huecos que no coincida con sus entradas. Los árboles de tamaño fijo no cambian. |
| Y5 | **Formato `10`**: los registros del diario y la organización de los nodos cambian, así que la versión de formato vigente pasa a ser `10` (J3) y un almacén de formato `9` es de formato anterior (D7 del padre raíz), con el mismo error que hoy. |
| Y6 | **Sin otros cambios visibles**: no hay operaciones, opciones, situaciones, clases de error ni datos de error nuevos. Lo que devuelven las operaciones, lo que es durable y cuándo (G2, J4, J10), lo que se descarta tras una caída (J9) y la información del almacén (P12) no cambian, salvo la versión de formato. Los archivos del diario siguen las reglas de J2 y E7; se llenan más despacio. |
| Y7 | **Rendimiento**: como S5 de las comprobaciones de la escritura, con al menos W1, W2, W3, W4, W7, W10, W12, W13, W14, W15, W17, W19 y W21 medidas. El cambio se acepta cuando W2, W14, W15 y W19 mejoran en las dos plataformas y ninguna carga medida empeora. Las ejecuciones completas de referencia son las del 2026-11-07 tras la reutilización de recursos de escritura (`docs/benchmark/2026-11-07-*-write-reuse.md`). |

### Estado de partida

Verificado el 2026-11-08 en el código, en los perfiles de ese día y en el plan de rendimiento:

| Aspecto | Situación |
|---|---|
| Registros del diario | Cada confirmación guarda, de cada página que modificó, su número y la página entera sellada; el conjunto se comprime cuando ocupa menos, y se guarda sin comprimir si no. Las páginas de un valor fuera de su hoja van enteras, marcadas para no comprobarse una a una. |
| Recuperación | Comprueba cada página de un registro antes de escribir ninguna y la escribe entera en su sitio, salvo las de los valores fuera de su hoja, que no comprueba. Una página de `krummi.data` cortada por un traslado se sobrescribe entera. |
| Borrar en un nodo | Mueve las entradas para que ocupen el final de la página sin huecos; un nodo con huecos no pasa la comprobación de organización. Sustituir una entrada por otra de distinto tamaño, en cualquier árbol con entradas de tamaño variable, la borra y la vuelve a insertar; con el mismo tamaño, la cambia en su sitio. |
| Coste medido | Según O8 del plan, compresión del diario: 36–40 % de W2, W14, W15 y W19 y 23 % de W21. Perfiles en Linux del 2026-11-08: la confirmación es el 61 % de W21, y en W17 preparar el registro es el 30 %. |
| Formato | Versión `9`. |
| Voron (referencia) | Según el código de RavenDB 7.2.7 que revisó la investigación del 2026-11-08: guarda en su diario las diferencias de cada página con su versión anterior, o sus partes que no son ceros si es nueva, y las comprime. |

## Actores

- **Desarrollador integrador**: lee y escribe como hasta ahora; un almacén de formato `9` debe recrearse (Y5).
- **Mantenedor**: ejecuta las pruebas, las de caídas y las ejecuciones de una sola carga y completas de Y7 en Windows
  y en Linux.

## Flujo

1. Una escritura modifica páginas: cada una parte de su última versión confirmada o se crea.
2. Al confirmar, el diario recibe de cada página lo que dice Y1.
3. El traslado escribe las páginas enteras en `krummi.data`, como hoy.
4. Tras una caída, la apertura aplica lo que el diario guarda desde lo trasladado, en orden, y comprueba cada página
   que aplicó (Y2, Y3).
5. Borrar una entrada o cambiar su tamaño deja un hueco; el nodo se reorganiza solo cuando hace falta (Y4).

## Validaciones y mensajes

No hay mensajes, clases de error ni datos de error nuevos.

| Situación | Resultado |
|---|---|
| Abrir un almacén de formato `9` | Falla como hoy con un almacén de formato anterior (Y5). |
| Una página aplicada en la recuperación no pasa su comprobación de integridad | La apertura falla con la clase de daño y el número de la menor de esas páginas (Y3). |
| Un nodo con huecos que no se solapan con sus entradas | Pasa la comprobación de organización (Y4). |
| Un nodo con entradas que se solapan, que se salen de la página o que superan sus tamaños máximos | Falla con la clase de daño y el número de su página, como hoy (Y4). |

## Salida

Nada de lo que devuelve una operación cambia de forma. La información del almacén (P12) informa la versión de formato
`10` (Y5).

## Reutilización

- La compresión del diario, la comprobación de integridad de cada registro y la detección de daño del diario (J9,
  J16).
- El orden de los registros y la escritura agrupada (E1–E15).
- La comprobación de integridad de las páginas (G6), para comprobar las páginas aplicadas.
- Las ejecuciones de una sola carga y las completas del plan de rendimiento, para Y7.

## Fuera de alcance

- Guardar diferencias en el área temporal o en `krummi.data`: las versiones del área temporal y el traslado siguen con
  páginas enteras.
- Huecos en los árboles de tamaño fijo.
- Borrar los bytes de las entradas borradas o de los huecos.
- Reorganizar los nodos con huecos en segundo plano o al trasladar.
- Migrar almacenes de formato `9` (D7).
- Reparar en la recuperación una página de `krummi.data` dañada (Y3), o comprobar y reconstruir las páginas en memoria
  antes de escribirlas.
- Comprimir con otro algoritmo, o elegir la compresión por registro o por página.
- Cambiar cuándo se traslada, cuándo se reescribe el encabezado o cuándo se borran los archivos del diario (J5, J6).
- La compresión de páginas hoja (5.6), el cifrado (módulo 9) y lo que el backup, la compactación y la verificación
  (módulos 10 a 12) hagan con los huecos: cada uno lo decide.
- La copia en el área temporal de las páginas de un valor fuera de su hoja y las demás optimizaciones del plan de
  rendimiento.

## Dejado a la implementación

- Cómo se calculan y se codifican los bytes guardados: su granularidad, cómo se agrupan y la organización de cada
  registro, dentro de Y1.
- Cómo encuentra la confirmación la última versión confirmada de cada página.
- Cómo aplica la recuperación lo guardado y cómo recuerda qué páginas comprobar al terminar, dentro de Y2 e Y3.
- Cómo sabe un nodo cuánto ocupan sus huecos y cómo se reorganiza.
- Cómo las pruebas simulan una página cortada por un traslado, miden lo que se guarda en el diario y fuerzan una
  reorganización en cada borrado para comparar.

## Criterios de aceptación

Valen los de los ascendientes, que deben seguir pasando sin cambios salvo los que nombran la versión de formato, que
pasa a `10`, incluidos los de caídas y recuperación del diario, AC10 del padre raíz y los de daño de los árboles.
Cada criterio se comprueba en un almacén en disco; los que no tratan del diario, también en modo temporal. Las pruebas
usan el conocimiento de la implementación para medir lo que se guarda en el diario, para simular caídas y páginas
cortadas, para alterar bytes y para forzar una reorganización en cada borrado.

1. **AC-Y1** — Dado un árbol normal con una hoja a medio llenar de entradas de clave de 16 bytes y valor de 100
   bytes, cuando tres confirmaciones separadas cambian el valor de una entrada por otro del mismo tamaño, guardan una
   entrada nueva sin dividir la hoja y borran una entrada del medio, entonces lo que el diario guarda de esa hoja en
   cada una, antes de comprimir, ocupa menos de una cuarta parte de una página; y lo que guarda de una página que una
   confirmación crea con una sola entrada, también (Y1).
2. **AC-Y2** — Dada una secuencia fija de confirmaciones que modifican varias veces las mismas páginas, crean páginas
   nuevas, liberan y reutilizan otras, guardan y sustituyen valores fuera de su hoja y crean una página toda de ceros,
   cuando el proceso cae sin trasladar y se reabre, entonces cada clave tiene su último valor confirmado y cada página
   de `krummi.data`, salvo las del encabezado, es igual a la que deja la misma secuencia sin caída, con el traslado
   detenido hasta el cierre (Y1, Y2).
3. **AC-Y3** — Dada la secuencia de AC-Y2 con al menos tres versiones de varias páginas, cuando la prueba deja en
   `krummi.data` esas páginas con bytes elegidos al azar, con una semilla fija, entre la versión trasladada y cualquier
   versión posterior, también saltándose versiones, y la apertura recupera, entonces cada página es su última versión
   confirmada; también cuando la recuperación empieza desde la copia más antigua del encabezado y cuando una primera
   recuperación se corta a mitad (Y2).
4. **AC-Y4** — Dada una página con una confirmación pendiente en el diario, cuando la prueba altera en `krummi.data` un
   byte que lo guardado de esa página no incluye y la apertura recupera, entonces falla con la clase de daño y el
   número de esa página, y una segunda apertura falla igual (Y3).
5. **AC-Y5** — Dada una secuencia fija de guardados, borrados en medio y sustituciones por entradas de distinto tamaño
   en un árbol normal y en un árbol multivalor, cuando se ejecuta y se lee, entonces cada lectura devuelve lo
   confirmado, cada nodo pasa la comprobación de organización y algún nodo se reorganizó para usar sus huecos; y el
   número de páginas de cada árbol es el mismo que con la reorganización forzada en cada borrado (Y4).
6. **AC-Y6** — Dado un nodo con dos entradas que se solapan, o una que se sale de la página, o una que supera su tamaño
   máximo, o cuyo registro de huecos no coincide con sus entradas, cuando se lee, entonces falla con la clase de daño y
   el número de su página (Y4).
7. **AC-Y7** — Dado un almacén de formato `9`, cuando se abre, entonces falla como un almacén de formato anterior; y un
   almacén nuevo informa la versión de formato `10` (Y5).
8. **AC-Y8** — Dadas las ejecuciones de una sola carga de Y7, cuando se comparan, entonces W2, W14, W15 y W19 mejoran en
   las dos plataformas y ninguna carga medida empeora; y el informe de cierre incluye esas comparaciones y las de las
   ejecuciones completas con las de referencia (Y7).

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones correspondientes, también en sus
traducciones (`*.prd.en.md`):

- Diario y recuperación: J16 precisa qué se guarda de cada página (Y1); J8 cambia en cómo se aplica, en que se
  comprueba cada página aplicada al terminar y en que una página dañada se informa después de aplicar lo demás (Y2,
  Y3); J3, la versión de formato vigente pasa a ser `10` y un almacén de formato `9` es de formato anterior (Y5).
- Escritura agrupada: el punto de Fuera de alcance «Guardar en el diario solo las diferencias de cada página» entra
  con este cambio; E15, la versión de formato vigente pasa a ser `10`.
- Pager, P5: la versión de formato vigente pasa a ser `10`.
- Padre raíz: D7, la versión de formato vigente es `10` (Y5); G5, una recuperación que encuentra una página dañada
  falla después de modificar el almacén (Y3).
- Árboles B+ y árboles y operaciones por clave (A9): la organización de un nodo admite huecos; el daño de organización
  es un solape, una entrada fuera de la página o de sus tamaños, o un registro de huecos que no coincide (Y4).
- Comprobación de organización recordada: el Objetivo describe la organización de un nodo «sin huecos ni solapes»;
  pasa a «sin solapes», con huecos permitidos (Y4).
- Comprobación recordada de las versiones del área temporal, U4: la versión de formato vigente es `10`.

## Referencias

- Padre: [journal.prd.md](journal.prd.md) v1.0 (J2, J3, J8, J9, J16, J17)
- Escritura agrupada: [journal-group-commit.prd.md](journal-group-commit.prd.md) (E7, E15, Fuera de alcance)
- Raíz: [krummi.prd.md](krummi.prd.md) v1.1 (D7, G2, G5, G6, AC10)
- Relacionados: [pager.prd.md](pager.prd.md) (P5, P12, P16), [trees.prd.md](trees.prd.md),
  [trees-keys.prd.md](trees-keys.prd.md) (A9), [trees-node-check-cache.prd.md](trees-node-check-cache.prd.md),
  [pager-version-checks.prd.md](pager-version-checks.prd.md) (U4), [pager-writer-checks.prd.md](pager-writer-checks.prd.md)
  (S5), [trees-multivalue.prd.md](trees-multivalue.prd.md), [trees-fixed-size.prd.md](trees-fixed-size.prd.md),
  [benchmark.prd.md](benchmark.prd.md) (K7, K13)
- Plan de rendimiento: [docs/performance/plan.md](../../performance/plan.md), O8
