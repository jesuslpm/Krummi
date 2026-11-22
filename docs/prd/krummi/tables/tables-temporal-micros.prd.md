# Valores temporales en microsegundos — PRD

**Versión**: 1.0
**Fecha**: 2026-11-22
**Documento padre**: [Tablas con esquema de Krummi — PRD](tables-schema.prd.md) (v1.0)

## Objetivo y contexto

Que la aplicación dé y lea los valores `time`, `timestamp` y `duration` en la misma unidad en que Krummi los guarda:
**microsegundos**. Hoy los da y los lee en nanosegundos, pero Krummi los guarda redondeados al microsegundo, así que un
valor leído es siempre un múltiplo de 1.000 ns y los límites publicados de los dominios están en microsegundos, otra
unidad que la del valor.

**Por qué**: las dos unidades despistan y no aportan nada. La única razón del nanosegundo era que E8 pudiera redondear
al microsegundo una fracción menor; con el microsegundo como unidad del valor no hay fracción que redondear. El
usuario: «No entiendo que los tipos de datos la unidad del valor de time, timespan y duration sean nanosegundos y lo
almacenado sea microsegundos. Esto despista mucho y no se explica.» y «La razón me parece absurda, uso nanosegundos
para poder redondear a microsegundos y así cumplir la regla, si son todos microsegundos la regla de redondear a
microsegundos consiste en no hacer nada.»

Este PRD cambia E8 de su padre (el redondeo de los valores dados) y precisa N4 de las tablas, IX9 de los índices, ST2
de las estadísticas y JS7 de las columnas JSON (la unidad de los valores); todos llevan su anotación (*Anotaciones a
otros PRD*). Lo guardado no cambia.

### Decisiones de producto

| ID | Decisión |
|---|---|
| TU1 | **Una sola unidad**: el valor de un `time` es el número de microsegundos desde la medianoche; el de un `timestamp`, el número de microsegundos desde 1970-01-01T00:00:00Z, en UTC; el de un `duration`, un número de microsegundos con signo. Es la unidad en que Krummi lo guarda, y vale en todo lugar por donde un valor temporal entra o sale de Krummi: los valores de una fila al insertarla, actualizarla, insertarla o actualizarla, cargarla en masa y leerla; los valores por defecto de una columna; los límites de los intervalos de la clave primaria (E12) y de los índices (IX9); las claves de los índices y sus entradas; las lecturas tipadas de una fila y de una proyección; los valores de las estadísticas (los más frecuentes de una columna y de los prefijos de un índice, y los límites del histograma); y las condiciones de una estimación (ST2). Los límites publicados de los dominios de E8 conservan su valor, ya en microsegundos, y su tipo. |
| TU2 | **Enteros de 64 bits con signo** para los tres tipos, en todos los lugares de TU1: los dominios de E8 caben en ellos (el mayor, el de `duration`, llega a 922.337.203.685.477.580). `time` ya lo era; `timestamp` y `duration` dejan de ser enteros de 128 bits. Son enteros sin un tipo propio, como los días de `date`: la unidad la dicen la documentación y las guías (usuario, entre un entero y un tipo propio con constructor que obligue a nombrar la unidad: «Entero i64 sin más»). |
| TU3 | **Sin redondeo de los valores dados** (cambia E8 del padre): un valor dado ya está en microsegundos, así que se guarda tal cual. El dominio de E8 se comprueba sobre el valor dado, con las clases de error de hoy en cada lugar (*Validaciones y mensajes*). Los dominios y el redondeo de `decimal` no cambian. Una actualización con el mismo valor guardado sigue siendo idéntica (E10, E13). |
| TU4 | **El instante leído de un JSON** (precisa JS7): la lectura de un instante de un texto JSON sigue convirtiéndolo a UTC, redondeando su fracción de segundo al microsegundo con el empate hacia el valor mayor y comprobando el dominio de E8 después de convertir y de redondear, como hoy; devuelve microsegundos (`"2026-11-11T10:23:00.1234567Z"` da 1.794.392.580.123.457). Es el único redondeo temporal que queda: el de un texto con más de seis cifras de fracción. Una hora o una duración guardadas en un JSON, que la aplicación lee con el tipo más cercano y convierte (*Fuera de alcance* de JS7), se dan a Krummi también en microsegundos. |
| TU5 | **Lo guardado no cambia**: las filas, las claves, los índices, las estadísticas y los valores por defecto guardados ya están en microsegundos, así que sus bytes son los mismos, la versión del formato no cambia (D7 de la raíz) y un almacén escrito antes de este cambio se abre y se lee igual, con sus valores en microsegundos. |
| TU6 | **La forma de texto no cambia** (la de los datos de los errores, como la clave de **clave duplicada**, N10 de las tablas, y E17): una hora es `HH:MM:SS` seguida, si tiene fracción, de un punto y sus cifras de fracción sin los ceros finales (`09:00:00.5`, `23:59:59.999999`); un instante, `AAAA-MM-DDTHH:MM:SS` con su fracción igual y una `Z` (`1970-01-01T00:00:00Z`, `2026-11-11T10:23:00.123457Z`); una duración, su signo si es negativa, sus segundos enteros, su fracción igual y una `s` (`-1.5s`, `0.000001s`). |
| TU7 | **Mediciones** (N11 de las tablas y su anotación del 2026-11-20): las cargas afectadas son las de tablas, W23–W32, porque todas escriben o leen la columna `timestamp` de sus filas. Las cargas dan los mismos datos que antes del cambio; sus tiempos son lo que se mide. |
| TU8 | **Documentación**: las guías de uso y la documentación del API explican una sola unidad, el microsegundo, y dejan de justificar el nanosegundo. |

### Estado de partida

- Los valores `time`, `timestamp` y `duration` que la aplicación da y lee son nanosegundos: `time` en un entero de 64
  bits, `timestamp` y `duration` en enteros de 128 bits. Las lecturas tipadas de una fila y de una proyección devuelven
  nanosegundos.
- Krummi los guarda en microsegundos, en enteros de 64 bits, en las filas, los valores por defecto, las claves de la
  clave primaria y de los índices y las estadísticas; al escribirlos redondea al microsegundo más cercano (E8) y al
  leerlos multiplica por 1.000.
- Los límites publicados de los dominios de E8 están en microsegundos, en enteros de 64 bits.
- La lectura de un instante de un JSON ([tables-json.prd.md](tables-json.prd.md), JS7) devuelve nanosegundos.
- La forma de texto de los valores temporales es la de TU6.
- Las cargas de tablas del banco (W23–W32) escriben y leen una columna `timestamp`, y W32 recorre un índice por ella.
- Las guías de uso explican hoy las dos unidades.

## Actores

- **Aplicación**: da y lee valores temporales en microsegundos.
- **Krummi**: los comprueba contra su dominio y los guarda tal cual.

## Flujo

1. La aplicación da un `time`, un `timestamp` o un `duration` en microsegundos: en una fila, un valor por defecto, un
   límite de un intervalo o una condición de una estimación.
2. Krummi comprueba el dominio de E8 sobre ese valor y lo guarda sin cambiarlo.
3. La aplicación lo lee en microsegundos: el mismo valor que dio.

Lo guardado antes se lee en microsegundos: el mismo instante, en otra unidad. Una aplicación que pase de nanosegundos
a microsegundos convierte sus valores así:

- **Un valor leído de Krummi antes de este cambio** es un múltiplo de 1.000 ns, así que dividirlo entre 1.000 es exacto
  y da el valor guardado.
- **Un valor original de la aplicación**, el que daba en nanosegundos, se convierte como lo guardaba Krummi: al
  microsegundo más cercano, con el empate hacia el valor mayor. Dividirlo entre 1.000 truncando puede dar un
  microsegundo menos que el guardado: un `timestamp` dado como 1.500 ns se guardaba como 2 µs, y 1.500 entre 1.000
  truncando da 1 µs. Un valor por defecto convertido así es distinto del guardado y, al abrir con ese esquema la tabla
  que ya existe, da esquema distinto (E6).

Un valor que la aplicación siga dando en nanosegundos se interpreta como microsegundos: un instante actual queda fuera
del dominio de `timestamp` y falla; una hora de más de 86.399.999.999 también; una hora menor o una duración dentro del
dominio no fallan y se guardan mil veces mayores. Un valor por defecto que siga en nanosegundos falla con esquema no
válido si queda fuera de su dominio (E5); dentro, al abrir con ese esquema una tabla que ya existe, da esquema distinto
(E6) salvo cuando es el mismo en las dos unidades, como el 0.

## Validaciones y mensajes

| Situación | Regla | Mensaje |
|---|---|---|
| Un `time`, `timestamp` o `duration` dentro de su dominio de E8 | Se guarda o se usa tal cual (TU3) | Ninguno |
| Uno fuera de su dominio, en el valor de una fila | E8, E17 | Valor no válido, regla fuera de dominio, como hoy |
| Uno fuera de su dominio, en el valor por defecto de una columna | E5 | Esquema no válido, como hoy |
| Uno fuera de su dominio, en un límite de un intervalo de la clave primaria o de un índice | E12, IX9 | Valor no válido, como hoy |
| Uno fuera de su dominio, en una condición de una estimación | ST2 | Valor no válido, como hoy |
| Un valor de otro tipo en una columna temporal | N4 | Valor no válido, regla tipo distinto, como hoy |
| Un instante JSON con más de seis cifras de fracción | TU4 | Ninguno: se redondea al microsegundo, como hoy |
| Un instante JSON fuera del dominio tras convertir y redondear | JS7 | Ninguno: la lectura da nulo, como hoy |
| Los demás tipos (`date`, `decimal` y el resto) | Sin cambios | Los de hoy |

Los extremos del rango de los enteros de 64 bits son valores como los demás: fuera de cada dominio, fallan como su
lugar indica. No hay mensajes nuevos ni cambia ningún mensaje existente.

## Salida

- Las lecturas tipadas de `time`, `timestamp` y `duration` devuelven enteros de 64 bits con signo en microsegundos: la
  hora 09:00:00.5 se lee 32.400.500.000; el instante 1970-01-01T00:00:01Z, 1.000.000; la duración de −1,5 s,
  −1.500.000.
- La lectura de un instante de un JSON devuelve un entero de 64 bits con signo en microsegundos (TU4).
- La forma de texto no cambia (TU6).
- Los demás valores y salidas conservan su forma.

## Reutilización

- Los dominios de E8, su comprobación y las clases de error de cada lugar.
- El formato guardado de 7.1, que ya está en microsegundos (TU5).
- La lectura de un instante de un texto JSON de JS7, con su redondeo al microsegundo (TU4).
- Las mediciones de N11 con su anotación del 2026-11-20.

## Fuera de alcance

- Precisión por debajo del microsegundo, en el valor o en lo guardado.
- Cambiar la unidad de `date`, los dominios de E8 o los límites publicados de los dominios.
- Conversiones desde o hacia los tipos de tiempo de la biblioteca estándar de Rust o de otras bibliotecas.
- Zonas horarias guardadas (N4).
- Cualquier cambio del formato guardado (TU5), del cálculo de las estadísticas o de las estimaciones.
- Los documentos de diseño de las tablas, que siguen su propia convención.

## Dejado a la implementación

- Nada observable: las reglas de este PRD fijan la unidad, el tipo, el redondeo, los errores, lo guardado y la forma de
  texto.

## Criterios de aceptación

1. **AC-TU1** — Dada una tabla con columnas `time`, `timestamp` y `duration` y un índice sobre cada una, cuando se
   escriben valores en microsegundos en los extremos de cada dominio de E8 y en un punto intermedio, insertando,
   actualizando, insertando o actualizando y con una carga masiva, y se leen por la clave, por un intervalo de la clave
   primaria, por cada índice, con las lecturas tipadas de la fila y con una proyección, entonces cada lectura da
   exactamente el valor dado; y un valor una unidad más allá de cada extremo, y los extremos del rango de los enteros de
   64 bits, fallan con valor no válido y la regla fuera de dominio, sin efecto.
2. **AC-TU2** — Dados los valores de AC-TU1, cuando se usan como valor por defecto de una columna añadida, entonces una
   fila escrita antes de añadirla lo lee en microsegundos, y uno fuera de su dominio falla con esquema no válido; cuando
   se usan como límites de un intervalo de la clave primaria y de un índice, entonces dan las filas de esos
   microsegundos, y fuera de su dominio fallan con valor no válido; cuando se usan como condiciones de una estimación
   sobre una tabla que se lee entera, entonces la estimación y las estadísticas de la columna y de los prefijos del
   índice son las mismas que antes del cambio sobre los mismos datos, con sus valores en microsegundos.
3. **AC-TU3** — Dados los lugares de TU1, cuando una aplicación da y lee un `time`, un `timestamp` y un `duration`
   (los valores de una fila, las lecturas tipadas de una fila y de una proyección, las entradas de un índice, los
   valores de las estadísticas y el instante leído de un JSON), entonces son enteros de 64 bits con signo (TU2).
4. **AC-TU4** — Dados los bytes de una fila con valores temporales, de su clave primaria, de la entrada de un índice
   sobre una columna temporal, de un valor por defecto temporal y de las estadísticas de esa columna, guardados por
   Krummi antes de este cambio, cuando se escriben los mismos instantes en microsegundos después del cambio, entonces
   los bytes guardados son los mismos y la versión del formato es la misma; y un almacén escrito antes de este cambio
   se abre y da sus valores en microsegundos (TU5).
5. **AC-TU5** — Dado un documento JSON, cuando se leen como instantes `"2026-11-11T10:23:00.1234567Z"`,
   `"2026-11-11T00:30:00+01:00"`, `"1969-12-31T23:59:59.9999995Z"` y `"0001-01-01T00:59:59.9999996+01:00"`, entonces dan
   1.794.392.580.123.457, 1.794.353.400.000.000, 0 y −62.135.596.800.000.000 microsegundos (este último, el primer
   instante del dominio, al que lo lleva el redondeo); `"0001-01-01T00:59:59.9999994+01:00"` y
   `"9999-12-31T23:59:59.9999995Z"`, que convertidos y redondeados quedan fuera del dominio, dan nulo; y los casos que
   hoy dan nulo siguen dando nulo (TU4).
6. **AC-TU6** — Dados la hora 09:00:00.5, la hora 23:59:59.999999, el instante 0, el instante
   2026-11-11T10:23:00.123457Z y las duraciones de −1,5 s y de 1 µs, en microsegundos, cuando se piden en su forma de
   texto, entonces dan `09:00:00.5`, `23:59:59.999999`, `1970-01-01T00:00:00Z`, `2026-11-11T10:23:00.123457Z`, `-1.5s`
   y `0.000001s`; y cuando se inserta dos veces una fila con cada valor en una tabla cuya clave primaria es una columna
   de su mismo tipo (`time` para las horas, `timestamp` para los instantes, `duration` para las duraciones), entonces
   la **clave duplicada** muestra la clave con ese mismo texto entre paréntesis (`(09:00:00.5)`) (TU6).
7. **AC-TU7** — Dado el cierre de este PRD, cuando se revisa su informe, entonces incluye las mediciones de N11 con su
   anotación del 2026-11-20 en Windows y en Linux, con W23–W32 como cargas afectadas, y las comprobaciones de los datos
   de las cargas del banco (K4) dan lo mismo que antes del cambio (TU7).
8. **AC-TU8** — Dadas las guías de uso, cuando se leen, entonces explican una sola unidad, el microsegundo, y sus
   ejemplos pasan (TU8).

## Anotaciones a otros PRD

Este PRD cambia o precisa reglas de documentos anteriores, que llevan las anotaciones del 2026-11-22 (en sus
traducciones también):

- Tablas, N4: los valores de `time`, `timestamp` y `duration` que la aplicación da y lee están en microsegundos, la
  resolución con que se guardan, en enteros de 64 bits con signo (TU1, TU2).
- Tablas con esquema, E8: un valor temporal dado ya está en microsegundos y se guarda sin redondear; la frase del
  redondeo de un instante, una hora o una duración con fracción por debajo del microsegundo queda solo para el instante
  leído de un texto JSON (TU3, TU4), y la nota de la hora de −500 ns deja de aplicarse. AC6: sus valores temporales por
  debajo del microsegundo (0001-01-01T00:00:00.0000005Z y 9999-12-31T23:59:59.9999995Z) ya no se pueden dar; la parte de
  `decimal` no cambia.
- Índices secundarios, IX9, y estadísticas, ST2: un valor temporal de un límite o de una condición ya está en
  microsegundos y no se redondea (TU3).
- Columnas JSON, JS7: el instante leído se devuelve en microsegundos; una hora o una duración convertidas por la
  aplicación se dan en microsegundos (TU4).

## Referencias

- Padre: [Tablas con esquema de Krummi — PRD](tables-schema.prd.md) (7.1).
- Ascendencia: [Tablas de Krummi — PRD](tables.prd.md) y la raíz,
  [Krummi, motor de almacenamiento embebido inspirado en Voron — PRD](../krummi.prd.md).
- Relacionados: [Columnas JSON de las tablas — PRD](tables-json.prd.md) (JS7),
  [Índices secundarios de las tablas — PRD](tables-indexes.prd.md) (IX9),
  [Estadísticas de las tablas — PRD](tables-statistics.prd.md) (ST2).
