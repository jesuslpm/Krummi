# Comprobaciones recordadas de lo que escribe el motor — PRD

**Versión**: 1.0
**Fecha**: 2026-11-16
**Documento padre**: [Comprobaciones de la escritura — PRD](pager-writer-checks.prd.md) (v1.0)

## Objetivo y contexto

Hacer más baratas las actualizaciones dejando de repetir dos comprobaciones sobre páginas que el propio motor acaba
de escribir y cuyo contenido conoce:

- la **comprobación de integridad** (G6) de una página que el traslado (J5) escribe en su sitio del archivo de datos
  o, en modo temporal, de la memoria del almacén. Hoy esa página deja de estar recordada antes de escribirla (I4), y
  la primera lectura posterior calcula de nuevo su código y, como el código es otro, comprueba entera su organización;
- la **comprobación de organización** (C1) de una hoja en la que una escritura solo sobrescribió valores guardados en
  la hoja con otros del mismo largo. Hoy la versión que publica esa escritura es nueva para todas las comprobaciones
  (H2) y la primera lectura de otra transacción la comprueba entera.

Con este cambio, la página que el traslado escribe queda recordada con el código que la confirmación selló en ella,
como hace Voron, y una hoja en la que solo cambiaron valores del mismo largo conserva la comprobación de organización
que ya tenía.

**Por qué**: en `tabla-actualizacion` (W28) Krummi queda en 0,88–0,89 veces Voron en Windows (informes del banco del
2026-11-16), y casi toda lectura de hoja de una actualización de W28 vuelve a comprobar entera la hoja (ver
[Estado de partida](#estado-de-partida)). Voron no repite ninguna de las dos comprobaciones.

**Por qué ahora**: es lo que queda de la diferencia de W28 tras la ejecución del margen de escritura. El usuario
decidió el 2026-11-16 abordar las dos comprobaciones y aceptar sus riesgos (Q1, Q3, Q4).

Este PRD es un cambio de las comprobaciones de la escritura. Hereda todo lo del padre y sus ascendientes; aquí solo
cambia qué recuerda el motor de lo que él mismo escribe. Lo que cambia reglas anteriores está declarado en sus
decisiones y anotado en los documentos afectados (ver [Anotaciones a otros PRD](#anotaciones-a-otros-prd)).

### Decisiones de producto

| ID | Decisión |
|---|---|
| Q1 | **Integridad de lo que escribe el traslado** (cambia I1, I4 e I5 de la integridad recordada; decisión del usuario del 2026-11-16, «Sí, como Voron»). Las páginas que I4 deja de recordar antes de que el traslado las escriba (páginas enteras, con el código que la confirmación selló en ellas; no el encabezado, que tiene su comprobación, P6), salvo las de un valor fuera de su hoja, quedan recordadas como comprobadas, con ese código, en cuanto sus bytes quedan escritos en su sitio; vale también para las páginas que el archivo ganó al crecer. Un fallo posterior al llevar esos bytes al disco (J13) no las deja de recordar, porque lo que se lee es lo que se escribió. Si la escritura de una página falla o queda a medias (G12), esa página sigue sin recordar, como dice I4; las demás páginas de la misma escritura del traslado que sí quedaron escritas quedan recordadas. Las páginas de un valor fuera de su hoja siguen I4 sin cambios. Lo que escribe la recuperación al abrir no cambia: la memoria empieza vacía después (I5). |
| Q2 | **Organización de lo que escribe el traslado** (cambia C1, C2 y C3 de la comprobación de organización recordada y U1). Cuando el traslado escribe un nodo cuya versión, en ese momento, ya pasó la comprobación de organización en el área temporal (U1) o la conserva por Q3, la página queda recordada también en la memoria compartida de nodos recordados (C2), con el mismo código de Q1; los bytes son los mismos que se comprobaron. Si la versión no la había pasado, no se recuerda nada y la primera lectura desde el archivo de datos la comprueba, como hoy; una comprobación posterior de la versión en el área temporal no añade nada a C2. Las entradas que añade Q2 comparten C2 con las demás y pueden descartarlas (C5); si C2 descarta después una de ellas, la lectura siguiente vuelve a comprobar esa página. Lo que esto cueste a las lecturas lo mide Q7. |
| Q3 | **Hojas en las que solo cambian valores del mismo largo** (cambia H2 de la comprobación de los nodos después de confirmar y U1; decisiones del usuario del 2026-11-16, «Sí, solo valor igual» y «Sí, se encadena»). Vale para las hojas de todos los árboles. Una hoja cumple Q3 en una transacción de escritura cuando todo lo que la transacción hizo en ella, desde que obtuvo su copia hasta confirmar, fue sobrescribir el valor de entradas que ya tenía, guardado en la hoja, con otro del mismo largo, una o varias veces y aunque los bytes sean iguales; y cuando la página o la versión de la que obtuvo la copia, en el momento de obtenerla, tenía su organización comprobada en esa transacción o recordada (C2, U1, o conservada por Q3); una copia obtenida sin esa comprobación no cumple Q3. Cualquier otro cambio de la hoja en la transacción la descalifica, aunque el resultado final parezca el mismo: añadir o quitar una entrada (también quitar y volver a añadir la misma clave), un valor de otro largo, un valor fuera de la hoja, una división o una fusión. La versión que publica la confirmación de una hoja que cumple Q3 conserva la comprobación de organización: ninguna lectura de otra transacción la vuelve a comprobar, en el área temporal ni, después del traslado, en el archivo de datos (Q2). Una sobrescritura con los mismos bytes puede no publicar ninguna versión nueva (Y1); entonces no hay nada que conservar. Las demás comprobaciones de cada lectura de un nodo (C1) siguen igual. |
| Q4 | **Compilaciones de depuración**. En las compilaciones de depuración, toda lectura por otra transacción de una hoja que conserva su comprobación por Q3, en el área temporal o en el archivo de datos, comprueba además su organización, fuera de las comprobaciones que cuentan los criterios, como la anotación del 2026-11-09 a H1; si falla, la lectura falla con la clase de daño y el número de su página; si pasa, no se recuerda nada más. Q1, Q2 y Q3 valen igual en las dos compilaciones. |
| Q5 | **Garantía resultante** (amplía la excepción de G6 del padre raíz y los riesgos residuales de I8, S3, U2 y H3, anotados en ellos). Se mantiene que todo daño hecho con el almacén cerrado se detecta (AC10): al abrir, la memoria de lo recordado está vacía (I5). Cambian tres cosas, ante fallos que el motor no causa (P16) o errores del propio motor: (a) si los bytes de una página que el traslado escribió cambian después en el disco o en la memoria del almacén sin que el motor los escriba, ninguna transacción lo detecta hasta reabrir; (b) si los bytes de una versión cambian en el área temporal sin que el motor los escriba y el traslado la escribe, la primera lectura desde el archivo de datos ya no lo detecta, como decía U2, hasta reabrir; (c) si un error del propio motor deja inconsistente una hoja al sobrescribir un valor del mismo largo, o un fallo del hardware o de la memoria cambia los bytes de una hoja mientras conserva su comprobación por Q3, en el área temporal o en el archivo de datos, ninguna lectura lo detecta en las compilaciones de producción hasta reabrir, por muchas confirmaciones de Q3 que la encadenen. En los tres casos, una lectura puede devolver datos que no son los escritos o terminar con un pánico en lugar de un error de daño (como en I8), y si una escritura copia esa página y confirma, el daño queda sellado y ya no se detecta tampoco al reabrir (como en S3). Se acepta como riesgo residual, como en Voron; amplía también el de C7. |
| Q6 | **Sin cambios visibles**: como S4. No hay situaciones, clases de error, datos de error ni opciones nuevas; lo que se lee y lo que se confirma es lo mismo que hoy; la versión de formato no cambia. Lo que recuerdan Q1 a Q3 usa las memorias que ya existen (I6, C5, U1) y no reserva memoria que pueda faltar. |
| Q7 | **Rendimiento** (aceptación). Comparación de Krummi solo, el commit anterior al cambio contra el cambio, en la misma sesión, con el método de medición del plan de rendimiento, en Windows y en Linux; las cargas de menos de un segundo por repetición y las de varios hilos, en al menos diez rondas, y las demás en al menos tres. Una carga mejora cuando su mediana después supera a la de antes y la supera en la mayoría de las rondas; empeora cuando su mediana después queda por debajo de la de antes y por debajo en la mayoría de las rondas; si no, queda igual. El cambio se acepta cuando W28 mejora en las dos plataformas y no empeora ninguna de las cargas que escriben o leen junto a una escritura (W1–W6, W12–W15, W17, W19, W21, W23–W29) ni de las de lectura que más usan C2 (W7, W22, W26). La apertura (W16) no se mide: no lee páginas que el traslado haya escrito. Si no se cumple, el informe de cierre lo dice y el usuario decide; no cumplir el umbral (K7) no impide cerrar el cambio. |

### Estado de partida

Verificado el 2026-11-16 en el código y en la medición de la ejecución del margen de escritura:

| Aspecto | Situación |
|---|---|
| Escrituras del traslado | Antes de escribir una página en su sitio, el motor la deja de recordar (I4); la primera lectura posterior calcula de nuevo su código y, como el código es otro, comprueba entera su organización (C2 recuerda por número de página y código). |
| Versiones del área temporal | La confirmación las publica sin comprobación de organización recordada (H2); la primera lectura de otra transacción la hace entera y la recuerda en la versión (U1). |
| Actualizaciones de filas de tabla | Una actualización de una fila que no cambia su largo (módulo 7) sobrescribe su valor en la hoja sin cambiar el resto de la hoja. |
| W28 | Por actualización: dos lecturas de rama, ya recordadas, y una de hoja: una versión leída por primera vez (54 %), una página que el traslado acaba de escribir (43 %), una página que cambió la propia escritura (3 %), una hoja ya recordada (0,1 %). Razón con Voron: 0,88–0,89 en Windows y 0,96 en Linux (informes del banco del 2026-11-16). |
| Voron (referencia) | Según el código de RavenDB 7.2.7 que revisó la ejecución del margen de escritura: comprueba el código de cada página del archivo una vez por proceso, no vuelve a comprobar las que él mismo escribe y no comprueba las que lee del área temporal o del diario; no comprueba la organización de ninguna página en producción. |

## Actores

- **Desarrollador integrador**: lee y escribe como hasta ahora; no cambia nada de lo que usa.
- **Mantenedor**: ejecuta las pruebas, también en una compilación de producción, y las mediciones de Q7 en Windows y
  en Linux.

## Flujo

1. Una escritura sobrescribe en una hoja valores del mismo largo y confirma: la versión publicada conserva la
   comprobación de organización (Q3).
2. Otras transacciones leen esa versión en el área temporal sin comprobar su organización (Q3, U1).
3. El traslado la escribe en su sitio y, al terminar esa escritura, la página queda recordada con su código y su
   organización (Q1, Q2).
4. Las lecturas siguientes desde el archivo de datos no calculan su código ni comprueban su organización, hasta
   cerrar el almacén, hasta que el traslado la escriba otra vez o hasta que C2 la descarte.

## Validaciones y mensajes

No hay situaciones, clases de error ni datos de error nuevos. Lo que el motor deja de detectar está en Q5.

## Salida

Nada de lo que devuelve una operación cambia de forma (Q6).

## Reutilización

- La memoria de páginas recordadas de la integridad recordada (I1, I6, I7) y la memoria compartida de nodos
  recordados (C2, C5), sin cambios en su forma ni en su tamaño.
- La comprobación recordada de las versiones del área temporal (U1).
- Las reglas del traslado (J5, T14) y de las escrituras que fallan (G12).
- El método de medición del plan de rendimiento y las mediciones de Krummi solo de la ejecución del margen de
  escritura, para Q7.

## Fuera de alcance

- Recordar las páginas de un valor fuera de su hoja que escribe el traslado (siguen I4).
- Conservar la comprobación de organización de una hoja en la que cambió algo más que valores del mismo largo, o de
  una rama.
- Cambiar qué comprueban y qué escriben la apertura, la recuperación, la consulta de información del almacén, el
  backup, la compactación, la migración, la verificación o el diario, y la comprobación del encabezado (P6).
- La compresión de las páginas hoja (5.6) y el cifrado (módulo 9): deciden en sus PRD si usan lo que se recuerda.
- Detectar que otro programa, o un fallo del hardware o de la memoria, cambia una página que el traslado escribió
  (Q5, P16).
- Una opción pública para desactivar este cambio, y estadísticas de lo recordado en la información del almacén.
- Las demás optimizaciones pendientes del plan de rendimiento: no comprimir cuando no sirve, el avance entre entradas
  de un intervalo, la caché de hojas recientes y la alineación de los bucles.

## Dejado a la implementación

- Cómo sabe la confirmación que una hoja cumple Q3 (por ejemplo, una marca en la copia que borra cualquier otro
  cambio) y cómo pasa a la versión publicada.
- En qué momento exacto de la escritura de cada página por el traslado se recuerda, dentro de Q1.
- Cómo cuentan las pruebas las comprobaciones de integridad y de organización, cómo alteran bytes y cómo simulan el
  fallo de una escritura del traslado, sin una opción pública.
- Las cargas y las repeticiones de Q7 además de las que nombra.

## Criterios de aceptación

Valen los de los ascendientes, que deben seguir pasando sin cambios salvo los que este PRD sustituye o precisa (ver
[Anotaciones a otros PRD](#anotaciones-a-otros-prd)), incluido AC10 del padre raíz. AC-Q1, AC-Q2, AC-Q5, AC-Q6 y
AC-Q9 se comprueban en un almacén en disco y en modo temporal; AC-Q3 y AC-Q4, que simulan un fallo de escritura o
reabren el almacén, solo en disco. AC-Q7 vale solo en las compilaciones de depuración y AC-Q8 solo en las de
producción; los demás, en las dos, con las comprobaciones de Q4 fuera de las cuentas. Las pruebas usan el conocimiento de la implementación para contar las comprobaciones, para
colocar páginas en el archivo de datos o en el área temporal, para forzar el traslado, para leer una página del
archivo de datos y para alterar bytes. Las cuentas se hacen con un solo hilo y con sitio en C2 para recordar lo que
se lee. AC-Q10 es una medición, no una prueba.

1. **AC-Q1** — Dada una hoja de un árbol en el archivo de datos, cuando una escritura le añade una entrada y
   confirma, una transacción de lectura la lee desde el área temporal y el traslado la escribe en su sitio y la saca
   del área temporal, entonces una transacción de lectura nueva la lee sin calcular su integridad ni comprobar su
   organización y devuelve la entrada; y tras cerrar y reabrir el almacén (solo en disco), su primera lectura
   calcula su integridad y comprueba su organización (Q1, Q2, I5).
2. **AC-Q2** — Dadas una página del mapa de espacio libre y una rama que una escritura cambia, y un valor fuera de su
   hoja que guarda, cuando confirma y el traslado los escribe en su sitio, entonces la primera lectura posterior de
   la página del mapa y de la rama no calcula su integridad, y la del valor fuera de su hoja sí (Q1, I4).
3. **AC-Q3** — Dada una página que el traslado va a escribir, cuando la escritura de sus bytes en su sitio falla
   con un error del sistema (simulado), entonces una lectura de esa página desde el archivo de datos por la prueba
   calcula su integridad (Q1, G12).
4. **AC-Q4** — Dada una hoja que el traslado escribió y quedó recordada, sin versiones pendientes de trasladar,
   cuando la prueba altera con el almacén abierto un byte del valor de una de sus entradas sin romper su
   organización, entonces una transacción de lectura devuelve el byte alterado sin fallar; y tras cerrar y reabrir el
   almacén, la primera lectura de esa hoja falla con la clase de daño y el número de su página (Q1, Q5).
5. **AC-Q5** — Dadas hojas con varias entradas en un árbol normal, en uno de tamaño fijo y en la tabla de una
   actualización de filas que no cambia el largo de la fila, cuando una escritura sobrescribe en cada una dos
   valores con otros del mismo largo, uno de ellos dos veces, y confirma, y otra escritura vuelve a hacer lo mismo y
   confirma, entonces la primera lectura de cada versión publicada por otra transacción, desde el área temporal y,
   después del traslado, desde el archivo de datos, no cuenta ninguna comprobación de organización y devuelve los
   valores nuevos (Q2, Q3).
6. **AC-Q6** — Dada una hoja como en AC-Q5, cuando una escritura sobrescribe uno de sus valores con otro del mismo
   largo y además, en la misma transacción, (a) añade una entrada a esa hoja, (b) quita una, (c) quita y vuelve a
   añadir la misma clave con el mismo valor, (d) cambia otro valor por uno de otro largo, (e) guarda un valor fuera de
   la hoja, (f) la divide o (g) la fusiona, entonces la primera lectura de la versión confirmada por otra transacción
   comprueba su organización (Q3, H2).
7. **AC-Q7** — Dada una hoja como en AC-Q5 cuya copia la prueba altera, rompiendo su organización, después de
   sobrescribir un valor del mismo largo, cuando la escritura confirma sin volver a leerla y otra transacción la lee,
   entonces, en las compilaciones de depuración, la lectura falla con la clase de daño y el número de su página (Q4).
8. **AC-Q8** — Dados AC-Q1, AC-Q5 y AC-Q6, cuando se ejecutan en una compilación de producción, entonces sus cuentas
   son las mismas (Q3, Q4).
9. **AC-Q9** — Dados cuatro hilos con transacciones de lectura que leen claves de un árbol, al menos mil veces cada
   uno, mientras una escritura confirma sobrescrituras del mismo largo y otros cambios y el traslado las escribe,
   entonces cada lectura devuelve los datos de su instantánea y ninguna falla (Q1–Q3, I7).
10. **AC-Q10** — Dadas las mediciones de Q7, cuando se comparan, entonces W28 mejora en las dos plataformas y
    ninguna de las cargas que nombra Q7 empeora; y el informe de cierre incluye esas comparaciones y las ejecuciones
    completas del banco en las dos plataformas (Q7).

## Anotaciones a otros PRD

Este PRD cambia reglas de documentos anteriores, que llevarán las anotaciones correspondientes:

- Padre raíz, G6: la excepción de I8, ampliada por S3, U2 y H3, se amplía con Q5. AC10 no cambia.
- Integridad recordada: I1 («solo se recuerdan comprobaciones superadas») tiene la excepción de Q1: una página que el
  traslado escribe queda recordada sin calcular su integridad, porque su contenido es el que el motor selló; AC-I1 se
  precisa: la primera lectura de una página que el traslado escribió en la apertura en curso no calcula su
  integridad, y los valores fuera de su hoja y las demás páginas siguen como dice. I4 pasa a decir que la página que el traslado escribe queda recordada al terminar su
  escritura (Q1), salvo las de un valor fuera de su hoja; I5 («las páginas que el archivo gana al crecer empiezan sin
  recordar») y el punto de Fuera de alcance sobre ellas dejan de valer para las que escribe el traslado; el paso 3 del
  Flujo, AC-I4 («su integridad se comprueba una vez») y AC-I10 se precisan con Q1; AC-I5 y AC-I7 («una página que todavía
  no se leyó») se precisan: el contenido nuevo o la página no son una que el traslado escribió en esa apertura y
  quedó recordada (Q1); el riesgo residual de I8 se amplía con Q5.
- Comprobaciones de la escritura: el riesgo residual de S3 se amplía con Q5. AC-S4 («una página del archivo de datos
  que nadie leyó en la apertura se comprueba en su primera lectura») se precisa: salvo las que escribió el traslado
  en esa apertura (Q1). AC-S6 no cambia.
- Comprobación de organización recordada: C1 y C2 («solo se recuerdan comprobaciones superadas» en una lectura)
  tienen la excepción de Q2, que recuerda en C2 lo que pasó la comprobación como versión del área temporal o la
  conserva por Q3; la última frase de C3 («leer del archivo de datos un contenido que antes se leyó del área temporal
  no lo encuentra recordado»), el paso 4 de su Flujo, AC-C2 y AC-C3 se precisan con Q2; el riesgo residual de C7 se
  amplía con Q5. Q2 se apoya en C8: lo recordado vale mientras los bytes no cambian.
- Comprobación recordada de las versiones del área temporal: U1 («no se consulta ni se guarda en C2» y «no sirve para
  otra versión de la misma página, más nueva») tiene las excepciones de Q2 y Q3; U2 deja de valer en lo que dice de
  una versión alterada que el traslado escribe (Q5); el paso 4 de su Flujo, AC-U3 y AC-U6 se precisan con Q2 y Q3.
- Comprobación de los nodos después de confirmar: H2 («La versión que publica la confirmación es nueva: nada de lo
  comprobado de la página de la que se copió vale para ella») tiene la excepción de Q3; AC-H2 se precisa: el cambio
  que hace la prueba no es una sobrescritura de Q3; el riesgo residual de H3 se amplía con Q5.

## Referencias

- Padre: [pager-writer-checks.prd.md](pager-writer-checks.prd.md) v1.0 (S2–S4)
- Ascendientes: [pager-integrity-once.prd.md](pager-integrity-once.prd.md) v1.0 (I1–I9), [pager.prd.md](pager.prd.md)
  v1.0 (P6, P10, P12, P16), [krummi.prd.md](../krummi.prd.md) v1.1 (G6, G12, AC10)
- Relacionados: [pager-commit-checks.prd.md](pager-commit-checks.prd.md) (H1–H3),
  [pager-version-checks.prd.md](pager-version-checks.prd.md) (U1, U2),
  [trees-node-check-cache.prd.md](../trees/trees-node-check-cache.prd.md) (C1–C5),
  [journal.prd.md](../journal/journal.prd.md) (J5), [transactions.prd.md](../transactions/transactions.prd.md) (T14),
  [benchmark.prd.md](../benchmark/benchmark.prd.md) (K7), [journal-page-diffs.prd.md](../journal/journal-page-diffs.prd.md) (Y1),
  [tables.prd.md](../tables/tables.prd.md) (las actualizaciones de filas de AC-Q5)
- Medición: ejecución del margen de escritura,
  [write-margin.md](../../../../odd/done/pager/write-margin.md) (L3) y su
  [informe de cierre](../../../../odd/done/pager/write-margin-report.md)
