# Índices vectoriales aproximados — PRD

**Versión**: 1.2
**Fecha**: 2026-11-25
**Documento padre**: [Tablas de Krummi — PRD](tables.prd.md) (v1.0)

## Objetivo y contexto

Construir el PRD hijo **7.6** del [reparto del módulo de tablas](tables.prd.md#reparto-en-prd-hijos):

- un **índice vectorial aproximado** sobre una columna vector, de una clase distinta de los índices secundarios de 7.2;
- su **mantenimiento asíncrono** en segundo plano;
- una **búsqueda de los k vecinos más cercanos** que combina filtros relacionales con la distancia: primero filtra por
  las condiciones relacionales y después busca por distancia entre las filas que las cumplen;
- su medición (N11).

**Objetivo del usuario**, con sus palabras: «quisiera como objetivo ser capaz de combinar filtros normales relacionales
con filtros vectoriales, donde primero se filtre por los relacionales normales y luego se busquen en los vectores».

**Por qué ahora**: 7.5 y 7.7 están cerrados, y la anotación del 2026-11-10 al padre construye 7.6 después de 7.5 y antes
del módulo 17, para que las consultas puedan buscar vecinos aproximados desde el principio. El
[experimento del documento de diseño 12](../../../benchmark/2026-11-24-ann-experiment.md) midió el algoritmo contra sus
umbrales: con 1 M de vectores de 768 dimensiones y L2, IVF-SQ8 encuentra el 95,8 % de los 10 vecinos exactos en un p95
de 17,20 ms, un hilo, sondeando el 4 % de las listas. El usuario decidió seguir con IVF-SQ8: «Sigue con IVF-SQ8».

El diseño técnico está en el [documento de diseño 12](../../../design/tables/12-vector-indexes.md) (estructuras, el
registro de filas pendientes, la reconciliación, el trabajador, los estados, las garantías de las búsquedas, el
entrenamiento y la deriva del cuantizador) y en el [documento 5](../../../design/tables/05-vectors.md) (los tipos de
elemento, las métricas, los núcleos de distancia deterministas y la búsqueda exacta), que son el estado de partida
técnico de este PRD. Este PRD cambia tres partes del documento 12:

- **Los filtros.** El documento decidía entre la distancia exacta y el índice con una estimación del planificador, y
  con el índice ampliaba el sondeo y filtraba al reordenar. Aquí la búsqueda encuentra primero las filas que cumplen el
  filtro, elige el camino según cuántas pasan y, con el índice, solo considera esas filas (VI6).
- **Las reconstrucciones.** El documento las proponía explícitas, con indicadores que las recomiendan; aquí son
  automáticas, y la aplicación también puede pedirlas (VI9).
- **Los criterios de cambio a HNSW.** En el experimento se disparan dos: el producto escalar pasa el p95 por 0,9 ms a
  frecuencia base, y GloVe necesita el 14,7 % de las listas. El usuario, con esas cifras a la vista, mantuvo IVF-SQ8
  para todas las métricas.

Decisiones del usuario del 2026-11-25:

- el algoritmo, después del experimento: «Sigue con IVF-SQ8»;
- cómo se busca entre las filas que pasan el filtro: «Híbrido (Recomendado)», es decir, distancia exacta cuando pasan
  pocas filas y el índice, limitado a las que pasan, cuando pasan muchas (VI6);
- qué filtros acepta: «Lo de 7.3, módulo 17, IN y NOT IN» (VI7), con los nulos como en SQL;
- con menos de k candidatas en las listas sondeadas: «Ampliar el sondeo (Recomendado)» (VI6);
- los elementos: «f32, f16 e i8 (Recomendado)» (VI2);
- la distancia de L2: «distancia euclídea» (VI8);
- la búsqueda exacta: «Sí, pública (Recomendado)» (VI16);
- el mínimo para entrenar: «10.000 filas (Recomendado)» (VI4);
- las reconstrucciones: «Automática» (VI9);
- el turno y los hilos: «15 ms y 1 hilo (Recomendado)» (VI3, VI4);
- la referencia de comparación: «pgvector» (VI15);
- las demás propuestas del borrador, que siguen las reglas de 7.2 y 7.3 (VI1, VI9 a pedido, VI10 a VI13): «Aceptar
  todas (Recomendado)».

*Anotación del 2026-11-25, al empezar la ejecución (v1.1): la ejecución se hace en Kubuntu y no puede probar ni medir
en Windows (usuario: «Solo Linux, anotado»). Este PRD se valida solo en Linux x86-64 con glibc y Windows deja de ser un
criterio: las pruebas, AC-VI10, AC-VI11 y las mediciones de VI15 son de Linux, y el PRD se puede cerrar sin Windows. Es
una excepción a D3 y AC18 de la [raíz](../krummi.prd.md), que lleva su anotación.*

*Anotación del 2026-11-26, tras la revisión con Codex (v1.2), con dos decisiones del usuario:*

- *El límite de VI15 al p95 de insertar una fila con el índice (como mucho un 10 % por encima del de sin él) se mide
  con confirmaciones durables (usuario: «Solo durables (Recomendado)»). Con confirmaciones diferidas, la medida es
  informativa.*
- *VI12 se cumple tal como está escrito (usuario: «Reanudar al abrir» y «Leer el catálogo al abrir»). Al abrir el
  almacén, el trabajador arranca y lee el catálogo en segundo plano, también si no hay índices vectoriales. Por eso
  las pruebas de AC-C9 ([trees-node-check-cache.prd.md](../trees/trees-node-check-cache.prd.md)), AC-X19
  ([trees-fixed-size.prd.md](../trees/trees-fixed-size.prd.md)) y AC-Q1
  ([pager-written-pages.prd.md](../pager/pager-written-pages.prd.md)) esperan a que el trabajador termine antes de
  medir. La raíz del catálogo, que lee el trabajador, se comprueba en esa lectura. El texto de esos criterios no
  cambia.*

Este PRD hereda todo lo de sus ascendientes. Precisa o amplía reglas de su padre, de
[tables-schema.prd.md](tables-schema.prd.md) (7.1) y de [tables-indexes.prd.md](tables-indexes.prd.md) (7.2), que
llevan su anotación (*Anotaciones a otros PRD*).

En este documento, un **índice vectorial** es un índice vectorial aproximado; su **registro de pendientes** es la lista
de filas que cambiaron y que el índice aún no refleja; el **trabajador** es el hilo del almacén que lo pone al día; una
**generación** es una versión completa del índice, entrenada y construida; y el **filtro** de una búsqueda son sus
condiciones relacionales.

### Decisiones de producto

| ID | Decisión |
|---|---|
| VI1 | **Clase de índice**: un índice vectorial se crea sobre una columna vector de una tabla, con un nombre del mismo espacio que los índices secundarios de esa tabla, y una métrica (VI2). Divide los vectores en listas por su centroide más cercano (IVF) y guarda de cada fila un código de 8 bits por componente (SQ8). Una búsqueda sondea las listas más cercanas a la consulta y reordena los candidatos con la distancia exacta. No es un índice secundario (N7): no tiene clave de columnas, no admite columnas incluidas, no es único y no se recorre. Una tabla puede tener varios, también sobre la misma columna con métricas distintas, pero no dos iguales (misma columna y métrica). Sigue las reglas de 7.2 para los nombres (IX1), los modos de crear nuevo y crear si no existe (IX2; una definición es otra si cambia su columna o su métrica), borrar (IX4) y los cambios para T10 (IX13); cuenta para el máximo de 64 índices por tabla y para el tamaño del esquema (IX3). Describir (IX6) devuelve además la lista de los índices vectoriales de la tabla, aparte de la de los secundarios y vacía si no tiene, en el orden de sus nombres según B5, cada uno con su identidad, su nombre, su columna (identidad y nombre) y su métrica; crear o borrar uno cambia el estado de los índices de IX6, y su entrenamiento, su construcción y sus reconstrucciones no. Nombrar un índice vectorial en una operación de los índices secundarios (recorrer, leer sus estadísticas) falla como con un índice secundario que no existe. Una fila con el vector nulo no está en el índice y nunca es un resultado. |
| VI2 | **Elementos y métricas** (usuario: «f32, f16 e i8 (Recomendado)»): columnas de elementos `f32`, `f16` o `i8`, con la métrica L2 o producto escalar; en una columna con la métrica coseno (N4), la métrica del índice es coseno. Las columnas de elementos `bit` no admiten índice vectorial. Las distancias son las del documento 5, con sus núcleos deterministas: la búsqueda exacta da las mismas filas y distancias en todas las plataformas con los mismos datos, y la aproximada también con el mismo archivo de almacén. Dos índices construidos aparte sobre los mismos datos pueden diferir, porque su entrenamiento depende de la muestra. |
| VI3 | **Mantenimiento asíncrono**: una escritura que inserta, actualiza o borra una fila solo anota la fila en el registro de pendientes de cada índice vectorial de la tabla, en la misma transacción; una actualización la anota solo si cambia el vector. El trabajador, un hilo del almacén, toma las filas pendientes en lotes cortos y pone el índice al día en escrituras propias, que toman el turno de escritura (G4) como cualquier escritor, en su orden de llegada: una escritura de la aplicación puede esperar a un lote, y esa espera cuenta para su tiempo límite (T4), como con las estadísticas (N8). Un lote retiene el turno como mucho 15 ms, el umbral del documento 12 (usuario: «15 ms y 1 hilo (Recomendado)»), y sus confirmaciones no esperan al disco: una caída puede perder lotes, siempre los últimos, y su trabajo se repite al abrir. Un lote es una escritura propia del trabajador, con cambios para T10 y en el diario como cualquier otra; no cambia filas de la tabla, y como es otra transacción no termina las referencias de las de la aplicación (A10). Un lote preparado para un índice que ya no es el mismo (borrado, vaciado o vuelto a crear, o con otra generación publicada) se descarta sin escribir nada. El índice nunca deja de reflejar una fila confirmada: o la refleja, o la fila está en el registro de pendientes. |
| VI4 | **Crear, entrenar y construir**: crear el índice es una escritura corta que guarda su definición; no bloquea las escrituras de la aplicación. Después, el trabajador lo entrena con una muestra de las filas (centroides y cuantizador) y lo construye recorriendo la tabla en lotes, mientras la aplicación sigue escribiendo; las filas que cambian durante la construcción se recogen del registro de pendientes. Estados: **entrenando** (desde que se crea, o desde que se vacía la tabla, VI10), **construyendo** (después de entrenar), **listo** (al terminar de construir) y **listo y reconstruyendo** (VI9, de vuelta a listo al publicar). Entrena cuando la tabla tiene al menos 10.000 filas con vector (usuario: «10.000 filas (Recomendado)»); hasta entonces sigue entrenando. El entrenamiento usa un solo hilo, el del trabajador, y una muestra de como mucho 256 vectores por lista (el mismo «15 ms y 1 hilo (Recomendado)»). El número de listas sale de las filas con vector al entrenar: esas filas divididas entre 1.000 y redondeadas hacia abajo, hasta 1 M de filas, y su raíz cuadrada redondeada hacia abajo por encima. |
| VI5 | **Buscar**: dentro de una transacción de lectura o de escritura, sobre una tabla abierta, se piden los **k vecinos más cercanos** de un vector consulta, nombrando el índice vectorial, que decide la columna y la métrica, con un filtro opcional (VI7) y opcionalmente cuántas listas sondear primero (VI6). Devuelve hasta k filas visibles en la instantánea de la transacción que cumplen el filtro, cada una con su distancia exacta, de menor a mayor distancia, con los empates ordenados por clave primaria. k va de 1 a 10.000 (documento 5). Un resultado es aproximado solo por las listas que no sondea y por los códigos de 8 bits; el retraso del trabajador nunca oculta una fila, porque la búsqueda evalúa con la distancia exacta las filas del registro de pendientes. Mientras el índice está entrenando o construyendo (no hay ninguna generación lista), la búsqueda calcula la distancia exacta de todas las filas que pasan el filtro: el resultado es exacto. |
| VI6 | **Filtrar primero** («Híbrido (Recomendado)»): con un filtro, la búsqueda encuentra primero las filas que lo cumplen y después busca entre ellas por distancia. Sin filtro, todas las filas cumplen el filtro. En todo este PRD, las filas que **pasan** son las que cumplen el filtro y tienen el vector no nulo (VI1): son las que cuentan para el umbral y para k. Si pasan como mucho un **umbral de filas**, calcula la distancia exacta de todas y el resultado es exacto; si pasan más, recorre el índice vectorial considerando solo las filas que pasaron, y el resultado es aproximado. Si en las listas sondeadas hay menos de k candidatas que pasaron, sigue sondeando las listas siguientes en orden de cercanía hasta juntar k o recorrerlas todas, así que devuelve k filas siempre que pasen al menos k (usuario: «Ampliar el sondeo (Recomendado)»). El número de listas que pide la aplicación es el del primer sondeo; la ampliación vale también entonces. El umbral lo fija la implementación con una medición, en filas, y la búsqueda informa el camino que tomó: exacto, o aproximado cuando usó el índice, aunque haya sondeado todas las listas. En los dos casos solo devuelve filas que cumplen el filtro. |
| VI7 | **Condiciones del filtro** («Lo de 7.3, módulo 17, IN y NOT IN»): las de una estimación de 7.3 (igual a un valor en una columna, en un intervalo de una columna, igual en varias columnas, ST2), más **está en** una lista de valores (IN) y **no está en** una lista de valores (NOT IN) de una columna, unidas con Y. Los valores tienen el tipo y el dominio de su columna, como en ST2, y se comparan después de la normalización de N4 y de la intercalación de su columna, como en la clave de un índice (N5), así que un NaN en una lista encuentra las filas con NaN. Varias condiciones sobre la misma columna se cumplen todas, y un valor repetido en una lista no cambia nada. Los nulos siguen SQL (usuario: «Como en SQL: supongo que es así: un nulo en IN es como si no estuviera, un nulo en NOT IN siempre da NULL»): una fila con la columna nula nunca cumple una condición sobre esa columna; una igualdad con un nulo, o un intervalo con un nulo como límite, no deja pasar ninguna fila; un nulo en una lista IN no deja pasar nada por sí mismo; y un NOT IN cuya lista tiene un nulo no deja pasar ninguna fila. Es distinto de ST2, donde igual a nulo estima las filas nulas. Las demás condiciones (O, distinto de, rutas de un JSON, un umbral de distancia) llegan con el módulo 17, que podrá usar esta búsqueda con sus propios filtros. |
| VI8 | **Unidades de la distancia** (usuario, para L2: «distancia euclídea»): L2 devuelve la distancia euclídea, no su cuadrado; producto escalar devuelve el producto con el signo cambiado, para que menor sea más cercano; coseno devuelve uno menos el coseno. Son números `f64`. El orden y los empates se deciden con el valor que compara el documento 5 (el cuadrado, en L2), así que dos filas a distancias euclídeas que coinciden al redondear siguen en su orden. Es la pregunta 6 del documento 5, que estaba abierta. |
| VI9 | **Reconstruir** (usuario: «Automática»): el trabajador reconstruye el índice por sí solo cuando sus indicadores pasan sus umbrales: la fracción de códigos con algún componente fuera del rango del cuantizador, el tamaño de la lista mayor frente a la media, y las filas que tiene la tabla frente a las que tenía al entrenar, que cambian el número de listas (VI4). Una reconstrucción entrena y construye una generación nueva mientras las búsquedas siguen usando la anterior, y la publica de una vez: una búsqueda ve la generación anterior o la nueva, nunca una mezcla. Hay como mucho una reconstrucción en curso por índice, y solo se reconstruye un índice listo de una tabla con al menos 10.000 filas con vector. La aplicación también puede pedir una, en una escritura: vale cuando se confirma, sobrevive al cierre, y pedirla con otra en curso, con el índice entrenando o construyendo, o con menos de 10.000 filas no hace nada. |
| VI10 | **Cambios de la tabla**: renombrar la columna conserva el índice; quitar una columna con un índice vectorial falla con esquema no válido, como con un índice secundario (E5); vaciar la tabla quita las entradas y el registro de pendientes, y el índice vuelve a entrenando; borrar la tabla o el índice libera todas sus páginas en esa escritura, también una tabla que otra sobrescribe al renombrarse (E2); renombrar la tabla conserva sus índices vectoriales; la carga masiva anota cada fila cargada en el registro de pendientes, y una carga que falla entera deja el registro como antes (E13). |
| VI11 | **Información del índice**: una operación pública, en una transacción de lectura o de escritura y según lo que ve esa transacción, devuelve de un índice vectorial: su estado (entrenando, construyendo, listo, o listo y reconstruyendo); el número de listas de la generación que usan las búsquedas, o ninguno si no hay; el número de filas del registro de pendientes; y los indicadores de VI9 de esa generación. Un indicador que no se puede calcular es ninguno, como el número de listas: los tres, sin generación lista; la fracción de códigos fuera de rango, en una generación sin entradas; y la lista mayor frente a la media, cuando todas las listas están vacías. Un indicador que es ninguno nunca dispara una reconstrucción. La información del almacén (P12) no cambia. |
| VI12 | **Cerrar y abrir el almacén**: cerrar, o dejar de usar el almacén sin cerrarlo, detiene el trabajador: abandona un entrenamiento en curso y espera a que termine el lote que ya tiene el turno. Nada confirmado se pierde: el registro de pendientes, el estado y el avance de la construcción sobreviven al cierre, y el trabajador sigue al abrir, entrenando de nuevo si estaba entrenando. Un almacén temporal (G9) funciona igual, en memoria. |
| VI13 | **Fallos del trabajador**, como ST9: un fallo de un lote al leer (daño, entrada y salida, memoria agotada) o al guardar sin pasar el almacén a no aceptar escrituras (disco lleno de G7, tamaño máximo de P10) deja el índice como antes de ese lote, sin informar a nadie, y el trabajador lo reintenta en su siguiente pasada; las búsquedas siguen siendo correctas, porque las filas siguen pendientes. Un fallo de entrada y salida al confirmar pasa el almacén a no aceptar escrituras (T12), como el de cualquier escritor; la siguiente operación de la aplicación lo informa, y el trabajador se detiene. El espacio que ocupa el índice cuenta para el disco y el tamaño máximo como el de cualquier árbol. |
| VI14 | **Formato**: el cambio es incompatible (B12) y la versión del formato pasa de 15 a 16. |
| VI16 | **Búsqueda exacta** (usuario: «Sí, pública (Recomendado)»): la misma búsqueda de VI5, con los mismos filtros, pedida como exacta y nombrando una métrica en lugar de un índice: calcula la distancia exacta de todas las filas que pasan el filtro, nunca usa un índice vectorial y no necesita que exista. Admite todos los tipos de elemento, `bit` incluido con la distancia de Hamming, y las métricas de VI2 en los demás. Su resultado es siempre exacto, con el orden, los empates, las unidades y el máximo de k de VI5 y VI8. Es la búsqueda exacta del documento 5 que el padre deja «nunca usan el índice». |
| VI15 | **Medición**: no hay cargas nuevas del banco con umbral frente a Voron, que no tiene índices vectoriales en su capa de almacenamiento. La referencia de comparación es **PostgreSQL con pgvector** (usuario: «pgvector»), con su índice IVFFlat y sus búsquedas iterativas (`ivfflat.iterative_scan`, desde pgvector 0.8.0), en la misma máquina y con los mismos datos. Es una comparación informativa del informe de cierre, con un script y sin umbral: PostgreSQL no entra en el banco. Compara, en un hilo (`max_parallel_workers_per_gather = 0`), el p95 de Krummi y de pgvector con la misma recall@10 de al menos 0,95, sin filtro y con un filtro que deja pasar pocas filas y otro que deja pasar muchas, y el p95 de insertar una fila en una tabla con el índice. Informa aparte el p95 de una consulta vacía de PostgreSQL (`SELECT 1`), que mide lo que cuesta su protocolo, y las versiones de PostgreSQL y pgvector usadas. Instalarlos en la máquina de medición es parte de la ejecución. El informe de cierre mide, con los datos de Cohere del experimento (1 M × 768, L2), un hilo, caché caliente, sin pendientes y el trabajador sin trabajo, y en Linux (anotación del 2026-11-25), estos umbrales del documento 12: recall@10 de al menos 0,95 con un p95 de como mucho 20 ms, sin filtro; el p95 de insertar una fila en una tabla con el índice, como mucho un 10 % por encima del de sin él; y el tiempo que un lote retiene el turno, como mucho 15 ms (VI3). Un umbral que no se cumple deja el PRD sin cerrar hasta que el usuario decida. Con N11 desde 7.3, también una ronda de todas las cargas del banco antes y después del cambio, y de 10 a 20 rondas de las cargas de escritura de tablas (W23 a W32), que son las afectadas, y de las que se aparten más de un 3 %. |

### Estado de partida

Verificado el 2026-11-25 en el código, en los PRD de los ascendientes y en los documentos de diseño:

| Aspecto | Situación |
|---|---|
| Columnas vector | Existen desde 7.1, con elementos `f32`, `f16`, `i8` y `bit`, dimensión de 1 a 4.096 (múltiplo de 8 para `bit`) y la métrica coseno opcional, salvo en `bit`; una columna coseno guarda la norma junto al vector y no admite vectores cero (N4). |
| Distancias | No hay núcleos de distancia, búsqueda exacta de los k vecinos ni filtro por umbral de distancia en el motor: solo el cálculo de la norma de una columna coseno. Los núcleos del documento 5 existen fuera del motor, en el laboratorio del experimento. |
| Índices secundarios | Existen desde 7.2: se crean en una sola escritura que retiene el turno mientras recorre la tabla, se mantienen en cada escritura de una fila y no admiten un vector en su clave, aunque sí como columna incluida. Una tabla tiene como mucho 64. |
| Segundo plano | De las tablas, el almacén solo tiene un hilo, el de las estadísticas de 7.3, que toma el turno de escritura como cualquier escritor y se detiene al cerrar; los demás hilos del almacén (el del diario y el de transferencia) no toman el turno. |
| Filtros | La estimación de 7.3 acepta igualdad en una columna, intervalo de una columna e igualdad en varias columnas. No hay IN, NOT IN ni una búsqueda que filtre filas por condiciones: los recorridos son por intervalos de la clave primaria o de un índice. |
| Consultas | El módulo 17 (planificador y nodos de consulta) no existe. |
| Información del almacén | P12 informa la versión del formato, el tamaño de página, el tamaño, las páginas y las páginas libres; nada de tablas ni índices. |
| Formato | La versión vigente es 15. |
| Banco | 32 cargas; ninguna con vectores. |
| Medición | El experimento del documento 12 midió IVF-SQ8 en memoria: cumple los umbrales a 1 M × 768 con L2; con el producto escalar pasa el p95 por 0,9 ms a frecuencia base, y con GloVe necesita el 14,7 % de las listas. |

## Actores

- **Desarrollador integrador**: crea índices vectoriales, busca vecinos con filtros relacionales en Rust, consulta el
  estado de un índice y, si quiere, pide una reconstrucción antes de que el trabajador la haga solo.
- **Mantenedor**: ejecuta en Linux las pruebas y las mediciones de VI15 (anotación del 2026-11-25).

## Flujo

1. La aplicación crea un índice vectorial sobre una columna vector de una tabla, con una métrica (VI1, VI2).
2. El trabajador lo entrena y lo construye en segundo plano mientras la aplicación sigue escribiendo (VI4).
3. Cada escritura de una fila anota la fila como pendiente, y el trabajador pone el índice al día (VI3).
4. En cualquier transacción, la aplicación pide los k vecinos más cercanos de un vector, con un filtro relacional
   opcional (VI5–VI7); la búsqueda filtra primero y busca después.
5. El trabajador reconstruye el índice cuando sus indicadores pasan sus umbrales; la aplicación puede consultar su
   estado y pedir una reconstrucción (VI9, VI11).

## Validaciones y mensajes

Valen las de los ascendientes. Este PRD añade, solo en la biblioteca:

| Situación | Clase de error |
|---|---|
| Un nombre de índice que no mide de 1 a 1.024 bytes | Tamaño no válido (B7, B8), como en 7.2 (IX1) |
| Crear un índice vectorial sobre una columna que no existe, que no es vector, de elementos `bit`, con la métrica L2 o producto escalar sobre una columna coseno o coseno sobre una que no lo es, igual a otro índice vectorial de la tabla, o que pasa del máximo de índices | Esquema no válido, con la regla, como en IX3 |
| Crear nuevo un índice con un nombre que ya tiene un índice de la tabla; crear si no existe con ese nombre y otra definición | Árbol ya existente; esquema distinto, como en IX2 |
| Buscar con, pedir la información o pedir la reconstrucción de un índice vectorial que no existe, o buscar nombrando un índice secundario | Esquema distinto, con la tabla y el índice, como en IX9 |
| Buscar, pedir la información o reconstruir a través de una apertura de una tabla que la escritura borró | Árbol no encontrado (A6, E17) |
| Buscar sin vector consulta, con uno de otra dimensión o tipo de elemento que la columna, con NaN o infinitos, o con un vector cero con la métrica coseno | Valor no válido, con la regla de N4 |
| Buscar con k fuera de 1 a 10.000, con un número de listas a sondear igual a cero, o en la búsqueda exacta con una métrica que la columna no admite | Valor no válido |
| Un filtro con una columna que la tabla no tiene | Esquema distinto, con la tabla y la columna |
| Un filtro sobre una columna vector o JSON | Esquema no válido, regla columna vector o columna JSON, como en ST12 y JS10 |
| Un filtro con un valor de otro tipo o fuera del dominio de su columna, con una lista IN o NOT IN vacía, o con una igualdad en varias columnas con una columna repetida | Valor no válido |
| Quitar una columna con un índice vectorial | Esquema no válido, con el índice, como en IX5 |
| Un fallo al leer durante la búsqueda | Daño, entrada y salida o memoria agotada, con T11 en una escritura |

Un número de listas a sondear mayor que el de listas sondea todas. En la búsqueda exacta, y mientras el índice no está
listo, el número de listas a sondear no se usa. Las búsquedas y la información no son cambios para T10; pedir una
reconstrucción sí lo es, como analizar en una escritura (ST7), porque guarda la petición. Borrar un índice vectorial que
no existe no hace nada, como en IX4.

Una búsqueda sobre una tabla vacía, con un filtro que no deja pasar ninguna fila o mientras el índice no está listo no es
un error.

## Salida

El módulo no escribe nada en pantalla. Buscar, aproximada o exacta, devuelve una lista de hasta k resultados, vacía si
ninguna fila pasa, y una sola marca para toda la búsqueda que dice si el resultado es exacto o aproximado (VI6); la
búsqueda exacta y la aproximada mientras el índice no está listo dicen siempre exacto. Cada resultado lleva los valores
de la clave primaria de su fila, decodificados en una copia propia de la aplicación como en IX8, y su distancia, un
`f64` en las unidades de VI8; van de menor a mayor distancia, con los empates por clave primaria. Las filas se leen
después por su clave primaria. La información del índice devuelve los campos de VI11, en una copia propia de la aplicación. Las
demás operaciones no cambian su salida.

## Reutilización

- Los núcleos de distancia y la búsqueda exacta del documento 5, que entran en el motor.
- Las condiciones de la estimación de 7.3 (ST2) y las reglas de los valores de un intervalo de un índice (IX9).
- Los recorridos por un índice secundario (7.2) para encontrar las filas que cumplen el filtro.
- El turno de escritura y su cola (G4, T4), y el modelo del hilo de las estadísticas (ST6, ST10) para el trabajador.
- Las identidades de las tablas, las columnas y los índices (E2), para que el trabajador descarte un lote de un índice
  que ya no existe.

## Fuera de alcance

- Los índices vectoriales sobre columnas de elementos `bit` y la distancia de Hamming en un índice.
- HNSW y otros grafos: el experimento no los eligió (decisión del usuario).
- Las columnas de partición por inquilino (pregunta 3 del documento 12): el filtro primero las cubre (VI6).
- Columnas incluidas en un índice vectorial.
- Un tope del registro de pendientes que una búsqueda salte (pregunta 2 del documento 12): la búsqueda siempre evalúa
  las filas pendientes.
- Paralelismo dentro de una búsqueda.
- El planificador por coste, los nodos de consulta y las condiciones que no son de VI7 (módulo 17).
- Filtrar por un umbral de distancia: es exacto y del módulo 17.
- Cuantizaciones más compactas (PQ, RaBitQ), DiskANN y SPANN.
- Devolver las columnas de la fila en el resultado de la búsqueda: la aplicación las lee por la clave primaria.
- Mostrar o comprobar un índice vectorial desde la herramienta de línea de comandos: los módulos 10 a 12 lo tratan como
  árboles (N1).
- Cancelar una construcción o una reconstrucción en curso (borrar el índice sí la termina), e informar su avance.
- Informar los fallos del trabajador: se reintentan sin avisar, como los de las estadísticas (VI13, ST9).
- Una operación que estime la recall del índice: la aplicación la mide comparando con la búsqueda exacta (VI16).
- Frenar a los escritores cuando el registro de pendientes crece.
- Un tiempo límite o una cancelación propios de una búsqueda.
- Centroides por inquilino y dividir listas localmente.
- Configurar el tope del turno, los hilos o la memoria del trabajador, o el umbral de VI6.

## Dejado a la implementación

- Cómo se guardan el índice, el registro de pendientes y las generaciones en árboles, con el punto de partida del
  documento 12.
- El número de listas que sondea una búsqueda si no lo pide, el número de candidatos que reordena y el umbral de filas
  de VI6, siempre que se cumplan VI15 y los criterios de aceptación.
- Los umbrales de los indicadores que disparan una reconstrucción (VI9), fijados con mediciones.
- El margen del cuantizador y el número de filas de un lote, dentro de los 15 ms de VI3.
- Los nombres de las operaciones, los tipos y las clases de error, en inglés y con las convenciones de Rust (D8).
- Cómo se construyen los ganchos de prueba de los criterios.

## Criterios de aceptación

Valen los de los ascendientes.

1. **AC-VI1** — Dada una tabla con una columna vector de cada tipo de elemento admitido y con la métrica coseno, cuando
   se crea un índice vectorial con cada métrica admitida, entonces se crea; y cuando se crea con cada definición no
   válida de *Validaciones*, entonces falla sin efecto con su clase.
2. **AC-VI2** — Dada una tabla con un índice vectorial listo, cuando una escritura inserta, actualiza el vector o borra
   filas y otra no cambia el vector, entonces, antes de que el trabajador las procese, una búsqueda en una transacción
   posterior devuelve las filas nuevas, no devuelve las borradas y ve los vectores nuevos, con su distancia exacta, y la
   fila cuyo vector no cambió no queda pendiente.
3. **AC-VI3** — Dada una tabla de 100.000 filas, cuando se crea un índice vectorial mientras otro hilo escribe filas sin
   parar, entonces ningún lote del trabajador retiene el turno más de 15 ms según el gancho de pruebas que lo mide, el
   índice pasa por entrenando, construyendo y listo, y al terminar las
   escrituras y vaciarse el registro de pendientes, cada fila tiene en el índice la entrada que le corresponde según el
   gancho de pruebas que compara el índice con las filas.
4. **AC-VI4** — Dado un índice en cualquier estado, cuando el proceso cae a mitad del entrenamiento, de la construcción
   o de un lote, y el almacén se abre de nuevo, entonces el trabajador sigue, el índice llega a listo y coincide con las
   filas, y ninguna página queda sin usar ni liberar.
5. **AC-VI5** — Dado un índice listo, cuando se busca sin filtro con el número de listas igual al total y el gancho de
   pruebas que reordena todas las candidatas, entonces el resultado es el de la búsqueda exacta (VI16); cuando se busca
   con un número de listas menor, entonces cada fila devuelta es visible, lleva la misma distancia que le da la búsqueda
   exacta y el orden es el de VI5. La recall la mide AC-VI11.
6. **AC-VI6** — Dada una tabla con un índice secundario, cuando se busca con un filtro de cada clase de VI7, con uno que
   deja pasar menos filas que el umbral y con uno que deja pasar más (el umbral, fijado con un gancho de pruebas), entonces solo vuelven filas que cumplen el filtro;
   con pocas filas, el resultado es igual al de la búsqueda exacta sobre las filas que pasan y se informa exacto; con
   muchas, se informa aproximado.
7. **AC-VI7** — Dada una transacción de lectura abierta, cuando otra escritura cambia filas y el trabajador las procesa
   o reconstruye el índice y lo publica, entonces la búsqueda en la transacción abierta devuelve lo mismo que antes de
   esos cambios.
8. **AC-VI8** — Dado un índice listo, cuando se pide una reconstrucción, entonces las búsquedas siguen respondiendo con
   la generación anterior hasta la publicación y después con la nueva, y la información del índice informa sus
   contadores antes y después.
9. **AC-VI9** — Dada una tabla con un índice vectorial, cuando se vacía la tabla, se carga en masa, se renombra la
   columna, se intenta quitarla y se borra el índice o la tabla, entonces pasa lo que fija VI10, y en un almacén
   temporal lo mismo.
10. **AC-VI10** — Dado un mismo archivo de almacén con un índice listo, cerrado y abierto otra vez en Linux, cuando se
    hace la misma búsqueda aproximada y la misma búsqueda exacta, entonces las filas y las distancias son idénticas bit
    a bit; la búsqueda exacta sobre los mismos datos cargados en otro almacén también; y los núcleos de distancia dan los
    bits que fija una prueba dorada (anotación del 2026-11-25: sin Windows).
11. **AC-VI11** — Cuando se miden los umbrales de VI15, entonces cada uno cumple su cifra (recall@10 ≥ 0,95 con p95
    ≤ 20 ms; insertar con el índice ≤ 1,10 veces sin él; un lote ≤ 15 ms) en Linux (anotación del 2026-11-25), y el
    informe de cierre
    trae la comparación con pgvector.
12. **AC-VI12** — Dada una búsqueda con la métrica L2, producto escalar y coseno, cuando se comparan sus distancias con
    las de referencia de VI8 calculadas aparte, entonces son esas; y dos filas cuyas distancias euclídeas coinciden al
    redondear salen en el orden de sus cuadrados.
13. **AC-VI13** — Dado un filtro con cada condición de VI7 sobre una columna que admite nulos, con filas nulas y con un
    nulo en la lista IN, en la lista NOT IN y en una igualdad, cuando se busca, entonces pasan exactamente las filas que
    da SQL; y dada una tabla de prueba donde las filas que pasan están lejos de las listas que se sondean, cuando se busca
    por el índice, entonces vuelven k filas; y dado un filtro que cumplen k filas, una de ellas con el vector nulo,
    cuando se busca, entonces vuelven k − 1 filas, y esa fila no cuenta para el umbral de VI6; y cuando se borran una a
    una todas las filas de una tabla con el índice listo, entonces la información da ninguno en los indicadores de VI11
    que no se pueden calcular y no hay reconstrucción.
14. **AC-VI14** — Dado un fallo inyectado en un lote del trabajador al leer y al guardar, cuando se busca, entonces el
    resultado es correcto y el índice se pone al día después; y dado un almacén de formato 15, cuando se abre con esta
    versión, entonces falla con la clase de formato anterior (F11, B12); y dado un fallo de entrada y salida inyectado al
    confirmar un lote, entonces el almacén deja de aceptar escrituras y la siguiente escritura de la aplicación lo informa
    (T12).
15. **AC-VI15** — Dada una tabla con 9.999 filas con vector y un índice, entonces el índice sigue entrenando y la búsqueda
    se informa exacta; con la fila 10.000, el índice entrena, y la información del índice da el número de listas de VI4
    para 10.000, 1 M y 1.000.001 filas (este último con el gancho que cuenta las filas al entrenar).
16. **AC-VI16** — Dado un índice listo, cuando el gancho de pruebas baja los umbrales de VI9 por debajo de sus
    indicadores, entonces el trabajador reconstruye solo y publica una generación nueva; cuando se pide una
    reconstrucción con otra en curso, con el índice entrenando o con menos de 10.000 filas, entonces no pasa nada; y
    cuando se cierra el almacén con una reconstrucción pedida y se abre, entonces se hace.
17. **AC-VI17** — Dado un índice en cada estado, cuando se pide su información, entonces trae los campos de VI11 con sus
    valores, también sin generación lista; y cuando se cierra el almacén limpiamente en cada estado y se abre, entonces el
    índice sigue desde donde estaba, y Describir lo lista como fija VI1.

## Anotaciones a otros PRD

Escritas el 2026-11-25:

- [tables.prd.md](tables.prd.md):
  - la anotación de 7.6, *Contenido*: la búsqueda no es solo aproximada, es la híbrida de VI6, y hay una búsqueda exacta
    pública (VI16); «sus cargas del banco (N11)» deja de valer: no hay cargas nuevas, y la medición es la de VI15;
  - *Fija*: lo que fija queda fijado aquí; la información del almacén (P12) no cambia, y la del índice es una operación
    propia (VI11);
  - N11: la comparación de VI15 es con pgvector, sin umbral, en lugar de cargas frente a Voron;
  - *Fuera de alcance*: «Filtrar por un umbral de distancia» sigue en el módulo 17.
- [12-vector-indexes.md](../../../design/tables/12-vector-indexes.md): las tres partes que cambia este PRD (filtros,
  reconstrucciones y criterios de cambio a HNSW) y sus preguntas de producto, decididas aquí.
- [tables-indexes.prd.md](tables-indexes.prd.md): el máximo de 64 índices por tabla cuenta los vectoriales (VI1), y
  quitar una columna con un índice vectorial falla como con uno secundario (VI10).
- [tables-schema.prd.md](tables-schema.prd.md): vaciar la tabla (E3), quitar una columna (E5) y la carga masiva (E13)
  con un índice vectorial siguen VI10.
- [tables-statistics.prd.md](tables-statistics.prd.md): las condiciones de VI7 añaden IN y NOT IN y tratan los nulos
  como SQL, distinto de ST2; las estimaciones de 7.3 no cambian.
- [05-vectors.md](../../../design/tables/05-vectors.md): la pregunta 6 queda decidida por VI8.

## Referencias

- Padre: [tables.prd.md](tables.prd.md). Raíz: [krummi.prd.md](../krummi.prd.md).
- Hermanos: [tables-schema.prd.md](tables-schema.prd.md) (7.1), [tables-indexes.prd.md](tables-indexes.prd.md) (7.2),
  [tables-statistics.prd.md](tables-statistics.prd.md) (7.3).
- Diseño: [documento 12](../../../design/tables/12-vector-indexes.md), [documento 5](../../../design/tables/05-vectors.md).
- Medición: [experimento ANN](../../../benchmark/2026-11-24-ann-experiment.md).

## Tiempo de elaboración

Medido con el historial de la sesión de Claude Code. Si la persona tarda más de 10 min en responder, se cuentan 10 min y el resto se descarta como inactividad. Intervenciones: mensajes y respuestas de la persona después del pedido inicial.

| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |
|---|---|---|---|---|---|---|---|
| 9df599dc | 2026-11-24 05:42 UTC+00:00 | 25 h 37 min | 16 h 39 min | 8 h 58 min | 7 h 48 min | 1 h 11 min | 25 |

<!-- prd-time: [{"session": "9df599dc-6ef7-4fc3-8883-fd027ed6e6d1", "start": "2026-11-24T05:42:24.309000+00:00", "wall": 92205, "discarded": 59922, "agent": 28051, "human": 4230, "interventions": 25, "idle_minutes": 10}] -->
