# Lecturas de filas sin comprobar su contenido — PRD

**Versión**: 1.0
**Fecha**: 2026-11-11
**Documento padre**: [Tablas con esquema de Krummi — PRD](tables-schema.prd.md) (v1.0)

## Objetivo y contexto

Hacer que leer filas de una tabla cueste lo que cuesta en Voron: al leer una fila por su clave o al recorrer un
intervalo, Krummi deja de comprobar el contenido de la fila y se apoya en la comprobación de integridad de las páginas
(G6 de la raíz), que detecta los bytes alterados del archivo. Voron lee los campos de sus filas directamente de la
página, sin comprobar su contenido.

**Por qué**: la carga `tabla-intervalos` (W27) incumplía su umbral de K7 en Windows y en Linux (0,15 y 0,19 en el
cierre de 7.1). Con las optimizaciones posteriores, que no cambian nada observable, llega a 0,77 en Windows
comprobando las filas, y a 0,83 sin comprobarlas. El usuario: «Y además ecribe un PRD que anote K7 para las cargas de
tablas: Voron no comprueba sus filas y Krummi sí (E19). Deja de comprobar las filas.» y «Como dije antes, las páginas
ya se comprueban con xxhash, así que ¿por qué más comprobaciones?». Para la fila mal formada eligió «Pánico del
proceso», precisado después como «Pánico normal del hilo»; para las escrituras, «Solo lecturas»; y para una
transacción que sobrevive a un pánico capturado, «Sigue intacta».

Este PRD precisa E19 de su padre (lo que 7.1 trataba como daño de la página de una fila al leerla), añade una
excepción a G6 de la raíz y precisa K7 del banco; los tres llevan su anotación (*Anotaciones a otros PRD*).

### Decisiones de producto

| ID | Decisión |
|---|---|
| L1 | **Las lecturas no comprueban el contenido de la fila** (precisa E19 del padre; excepción a G6 de la raíz). Leer una fila por su clave y recorrer un intervalo, en los dos sentidos y en transacciones de lectura y de escritura, ya no comprueban el contenido de la fila: sus valores (el tipo y el dominio de E8, la precisión de un `decimal`, que un `bool` sea 0 o 1, la longitud de un vector, que un texto sea UTF-8 válido), dónde empieza y acaba cada valor de longitud variable, los bits de nulo, ni los elementos de su clave. Siguen fallando con daño de la página de la fila, en la lectura o en el paso del recorrido que la devuelve, como hoy: una fila de una versión de esquema que la tabla no tuvo o que falta en su historial; una fila cuya cabecera no es una que Krummi escribe (el ancho de sus desplazamientos, los bits reservados o de compresión); y una fila que no llega al final de su parte fija y de sus desplazamientos. Las columnas añadidas después de escribir la fila se leen con nulo o su valor por defecto, como hoy (N3), y el catálogo, los valores por defecto guardados en él y el historial se comprueban como hoy. El orden y los límites de un recorrido son los de los bytes de las claves guardadas, como hoy, también con una clave mal formada. |
| L2 | **Una fila mal formada en una lectura** (su contenido no es el que Krummi escribe, en una página cuya integridad pasó G6: un fallo de Krummi, un archivo alterado con el almacén cerrado que recalcula los códigos de integridad, o una página recordada alterada con el almacén abierto, ya excepción de G6) se devuelve sin error. Leer una de sus columnas, también una de la clave, puede devolver un valor que no es el escrito o terminar con un pánico normal del hilo que la lee: termina ese hilo, o el proceso si es el principal, y la aplicación puede capturarlo (decisión del usuario). Un texto que no es UTF-8 leído como texto termina siempre con un pánico; leído como sus bytes, se devuelve tal cual. El pánico ocurre al leer la columna, sin cambiar el almacén ni la transacción: si la aplicación lo captura, la transacción sigue intacta, de lectura o de escritura, y una de escritura puede seguir y confirmarse (decisión del usuario); si el pánico atraviesa la transacción, esta se descarta como una abandonada (T10) y, si es de escritura, su turno queda libre. Igual en todas las compilaciones del motor que deshacen la pila en los pánicos; con una que aborta en los pánicos, el pánico termina el proceso. |
| L3 | **Actualizar e insertar o actualizar siguen comprobando** la fila que ya existe y que reemplazan, y fallan con daño de su página si está mal formada, como hoy (E19, E17), así que no la reescriben con valores que no son los suyos. Dentro de una transacción de escritura, leer por la clave y recorrer siguen L1. Insertar, la carga masiva, borrar, borrar un intervalo, contar filas y las secuencias no leen el contenido de filas existentes y no cambian (E19). Una fila mal formada que otra escritura no lee, en una página que esa escritura reescribe, conserva sus bytes con el nuevo código de integridad de la página, como ya admite la anotación de G6 del 2026-11-06. |
| L4 | **K7 en las cargas de tablas que leen filas** (precisa K7 del banco para W26 y W27): Voron lee sus filas sin comprobar su contenido y, desde este PRD, las lecturas de Krummi tampoco lo comprueban, así que esas cargas comparan el mismo trabajo. Los umbrales no cambian. |

### Estado de partida

- 7.1 trata como daño de su página una fila leída cuyo contenido no es el que Krummi escribe: cada fila se comprueba
  entera en la lectura o en el paso del recorrido que la devuelve (E19 habla del daño de una página; la comprobación
  de la fila es de la implementación de 7.1).
- W27 tenía razones de 0,15 y 0,19 en el cierre de 7.1; tras las optimizaciones sin cambio observable, 0,77 en
  Windows comprobando las filas y 0,83 sin comprobarlas (una sola carga). W26 ya cumple. W28 se queda en 0,76
  comprobando o sin comprobar. Son contexto, no criterios: la aceptación es AC-L9.
- G6 de la raíz ya tiene excepciones aceptadas en las que un contenido alterado puede devolver datos que no son los
  escritos o terminar en un pánico (anotaciones del 2026-10-30, 2026-11-05, 2026-11-06 y 2026-11-09).
- El backup, la compactación, la migración y la verificación del almacén son de módulos futuros (10–12 de la raíz);
  el padre de 7.1 les pide tratar las páginas de las tablas como las de los árboles, sin leer el contenido de las
  filas, y este PRD no lo cambia.
- Ningún dato guardado cambia: el formato de las filas, de las claves y del almacén es el de 7.1 (E20).

## Actores

- **La aplicación** que lee filas de una tabla, por su clave o recorriendo intervalos, y que las escribe.
- **El banco comparativo**, cuyas cargas de tablas comparan Krummi con Voron (K7).

## Flujo

Leer, recorrer y escribir filas siguen como en 7.1 (E10–E12); solo cambia lo que una lectura comprueba (L1) y lo que
pasa al leer una fila mal formada (L2).

## Validaciones y mensajes

No hay mensajes nuevos. En las lecturas desaparece el fallo con daño de una fila cuyo contenido no es el que Krummi
escribe (L1, L2). Siguen fallando con daño, con los datos de E19: una página con la integridad dañada, un nodo mal
organizado, los casos de L1 que siguen comprobándose, el catálogo o el historial dañados, y, al actualizar e insertar
o actualizar, una fila existente mal formada (L3).

## Salida

La de 7.1 para las filas bien formadas. Una fila mal formada en una lectura sigue L2.

## Reutilización

- La integridad de las páginas y su integridad recordada (G6 y sus excepciones), que siguen siendo la defensa contra
  los bytes alterados del archivo.
- La comprobación de organización de los nodos de los árboles y la comprobación de las filas de 7.1, que sigue
  valiendo para actualizar e insertar o actualizar (L3).

## Fuera de alcance

- La carga `tabla-actualizacion` (W28), que incumple su umbral por el camino de escritura: un cambio aparte (K13).
  Las cargas W23, W24, W25 y W29 no leen filas y no cambian.
- Un árbol especializado de clave de tamaño fijo para claves de una columna.
- Fijar los procesos del banco a los núcleos de rendimiento en los procesadores híbridos: un cambio aparte del banco.
- Un modo que vuelva a comprobar el contenido de las filas en las lecturas, y una orden que verifique el contenido de
  las filas de un almacén.
- Las comprobaciones de las secuencias, del catálogo, de los valores por defecto y del historial, que no cambian.
- Los lectores de filas de PRD futuros: los índices secundarios (7.2), las estadísticas (7.3), las columnas JSON
  (7.5), las consultas (17), la interfaz C y el binding C# (18) y la importación y exportación (19), que fijarán si
  heredan L1 y L2.

## Dejado a la implementación

- Qué contenidos mal formados, aparte del texto que no es UTF-8 (L2), devuelven un valor que no es el escrito y
  cuáles terminan en un pánico, y en qué lectura de columna ocurre: los valores, sus desplazamientos (también uno que
  empieza después de acabar), los bytes que sobran tras el último valor, los bits de nulo (también uno puesto en una
  columna que no admite nulos) y los elementos de la clave (un elemento de otro tipo, truncado o sin su terminador).
- Cómo escriben las pruebas una fila mal formada en una página con la integridad correcta: es un gancho de prueba, no
  un contrato público.

## Criterios de aceptación

1. **AC-L1** — Dada una fila guardada con un texto que no es UTF-8 en una página con la integridad correcta, cuando se
   lee por su clave y cuando se recorre hacia delante y hacia atrás un intervalo que la incluye, en una transacción de
   lectura y en una de escritura, entonces la lectura y el paso del recorrido no fallan, sus demás columnas se leen con
   sus valores, leer ese texto como texto termina con un pánico y leerlo como bytes los devuelve tal cual; las pruebas
   pasan igual compiladas en depuración y en optimizado.
2. **AC-L2** — Dadas una fila con un `bool` que no es 0 ni 1, una con un elemento de la clave mal formado y una con un
   desplazamiento de un valor variable fuera de la fila, en páginas con la integridad correcta, cuando se leen por su
   clave o se recorren, entonces la lectura y el paso del recorrido no fallan, y el recorrido devuelve las filas del
   intervalo en el orden de los bytes de sus claves (lo que da leer la columna mal formada queda a la implementación,
   L2).
3. **AC-L3** — Dadas una fila de una versión de esquema que la tabla no tuvo, una con una cabecera que Krummi no
   escribe y una que no llega al final de su parte fija, en páginas con la integridad correcta, cuando se leen por su
   clave o se recorren, entonces fallan con la clase de daño y el número de la página de la fila; y una fila de una
   versión que falta en el historial de la tabla falla con la clase de daño y el número de la página del historial
   que debería tenerla. Todo como hoy.
4. **AC-L4** — Dada una fila existente mal formada en una página con la integridad correcta, cuando se actualiza o se
   inserta o actualiza, también después de leerla en la misma transacción, entonces falla con daño de su página, sin
   cambiar la fila, y la transacción sigue T11, como hoy (L3).
5. **AC-L5** — Dado el pánico de leer un texto que no es UTF-8: cuando la aplicación lo captura alrededor de esa
   lectura, entonces la transacción sigue intacta (en una de lectura, las demás lecturas funcionan; en una de
   escritura, puede seguir escribiendo y confirmarse, y se aplica lo que hizo); y cuando lo captura fuera de la
   transacción, que el pánico atraviesa, entonces una de lectura termina, una de escritura se descarta sin aplicar
   nada y su turno queda libre (T10), y en los dos casos otras lecturas, la siguiente escritura y el cierre del almacén
   funcionan.
6. **AC-L6** — Dada una página de filas alterada con el almacén cerrado, cuando se lee o se recorre una fila suya,
   entonces falla con daño (AC19 de 7.1 y AC10 de la raíz siguen valiendo).
7. **AC-L7** — Dado un almacén escrito por 7.1, cuando se abre y se leen sus tablas, entonces se leen las mismas filas
   con los mismos valores, sin migrar nada.
8. **AC-L8** — Dados E19 de 7.1, G6 de la raíz y K7 del banco, en el original y en su traducción, cuando se leen,
   entonces llevan la anotación de este PRD.
9. **AC-L9** — Dadas las ejecuciones completas del banco en Windows y en Linux, cuando se comparan con las del cierre de
   7.1, entonces W26 y W27 cumplen su umbral de K7 en las dos, y ninguna otra carga baja más de 0,05 su razón con
   Voron, salvo que la mediana de una medición de una sola carga, con sus repeticiones, la iguale a la del cierre de
   7.1 dentro de ese margen. Un umbral de W26 o W27 que siga incumplido no es un fallo de este PRD: sigue K13 y se
   informa en el cierre.

## Anotaciones a otros PRD

Este PRD cambia reglas de documentos anteriores, que llevan estas anotaciones del 2026-11-11 (en sus traducciones
también):

- [tables-schema.prd.md](tables-schema.prd.md), E19: *«Las lecturas por la clave y los recorridos ya no comprueban el
  contenido de la fila (sus valores, sus desplazamientos, sus bits de nulo y su clave); una fila mal formada en una
  página cuya integridad pasó G6 se devuelve sin error, y leer una de sus columnas puede devolver un valor que no es el
  escrito o terminar con un pánico; un texto que no es UTF-8 leído como texto termina siempre con un pánico. Siguen
  fallando con daño la versión de esquema desconocida o ausente del historial, la cabecera que Krummi no escribe y la
  fila que no llega al final de su parte fija; actualizar e insertar o actualizar siguen comprobando la fila que
  reemplazan (decisiones L1–L3 de [tables-row-reads.prd.md](tables-row-reads.prd.md)).»*
- [krummi.prd.md](krummi.prd.md), G6: *«Excepción a G6: leer por la clave y recorrer las filas de las tablas no
  comprueban su contenido. Una fila mal formada en una página cuya integridad pasó G6 (un fallo del motor, un archivo
  alterado con el almacén cerrado que recalcula los códigos de integridad, o una página recordada alterada con el
  almacén abierto) puede devolver datos que no son los escritos o terminar en un pánico en lugar de un error.
  Actualizar e insertar o actualizar siguen comprobando la fila que reemplazan; AC10 no cambia (decisiones L1–L3 de
  [tables-row-reads.prd.md](tables-row-reads.prd.md)).»*
- [benchmark.prd.md](benchmark.prd.md), K7: *«En W26 y W27, Voron lee sus filas sin comprobar su contenido y Krummi
  tampoco desde [tables-row-reads.prd.md](tables-row-reads.prd.md) (L4): las dos cargas comparan el mismo trabajo. Los
  umbrales no cambian.»*

## Referencias

- PRD padre: [tables-schema.prd.md](tables-schema.prd.md) (E10–E12, E17, E19, E20); su padre,
  [tables.prd.md](tables.prd.md) (N3); la raíz, [krummi.prd.md](krummi.prd.md) (G6, D11, D12, AC10).
- Banco: [benchmark.prd.md](benchmark.prd.md) (K7, K13).
- Transacciones: [transactions.prd.md](transactions.prd.md) (T10, T11).
- Por qué un texto mal formado termina siempre en pánico: se devuelve sin copia (D11) y sin código no seguro fuera del
  pager (D12), así que se valida al leerlo.
- Voron: `TableValueReader` en `src/Voron/Data/Tables/TableValueReader.cs` de RavenDB 7.2.7, que lee cada campo por su
  desplazamiento sin comprobarlo.
- Mediciones: `odd/tasks/table-row-reads.md`; informe de cierre de 7.1, `odd/tasks/tables-schema-report.md`.
