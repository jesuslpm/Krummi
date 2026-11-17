# Diario sin comprimir lo que no se comprime — PRD

**Versión**: 0.1 (borrador con preguntas abiertas para el usuario; no autoriza la implementación)
**Fecha**: 2026-11-17
**Documento padre**: [Diario de los cambios de cada página — PRD](journal-page-diffs.prd.md) (v1.0)

## Objetivo y contexto

Hoy el diario intenta comprimir lo que guarda de cada confirmación y lo guarda comprimido cuando así ocupa menos
(Y1: «Después se comprime como hoy cuando ocupa menos comprimido»). Cuando los datos no se comprimen, como los valores
grandes de bytes aleatorios de la carga W17 del banco, el tiempo de comprimir se pierde.

Ese tiempo es poco. En el perfil de W17 del plan de rendimiento, comprimir es alrededor del 8 % del tiempo, frente al
38 % de copias y el 13 % de poner páginas a cero. Un intento anterior (T2 de la ejecución de optimizaciones de
escritura) guardaba sin comprimir los registros de 4 MiB o más cuyas ocho muestras de 32 KiB se reducían menos de
1/32. No se llegó a medir su ganancia en W17 por separado: el 1,042 de W17 que se cita es de T1–T4 juntos, y el código
final, ya sin T2, midió W17 en 1,010–1,036 frente al anterior. La revisión de Codex mostró que un registro que se
comprime 31 veces quedaba sin comprimir, y se revirtió.

No hay ninguna regla que ahorre la compresión y guarde siempre lo mismo que hoy: saber si un registro se comprime
exige comprimirlo (L2 de la ejecución de las optimizaciones de la noche). Cualquier ganancia pide aceptar que el
diario guarde, en algunos casos, más bytes que hoy. Eso cambia el primer punto de J16 («ocupa menos que las páginas
que cambia cuando sus datos se comprimen bien»), Y1 y quizá AC-J15. La ganancia posible está acotada por ese 8 %.

**Recomendación de quien redacta:** no hacerlo (pregunta 1, opción a). La ganancia máxima es pequeña y no está medida,
y cada opción hace crecer el diario o cambia su formato.

### Decisiones de producto

Ninguna fijada. Ver *Preguntas abiertas*.

### Estado de partida

- Cada confirmación se guarda como un registro: lo que cambia de cada página (Y1), comprimido entero cuando así ocupa
  menos, o sin comprimir. La recuperación ya lee los dos.
- Las páginas de un valor fuera de su hoja se guardan enteras, sin sus partes que son ceros (Y1).
- W17 carga 4.096 valores de 64 KiB de bytes pseudoaleatorios, que no se comprimen, en confirmaciones de 256.
- El diario no cuenta para el tamaño máximo del almacén (J15), pero mientras una lectura larga detiene el traslado
  crece sin más límite que el disco (J5), y una confirmación que no cabe en el disco falla (J12, AC-J14).

## Preguntas abiertas

1. **¿Vale la pena?**
   - (a) No: el PRD queda archivado como decisión. Recomendada.
   - (b) Sí: elegir en la pregunta 2.
2. **¿Qué registros pueden guardarse con más bytes que hoy?**
   - (a) Los de valores grandes: las páginas de los valores fuera de su hoja se guardan siempre sin comprimir, y el
     resto del registro como hoy. Cambia el formato del registro: el formato pasa a `11` y los almacenes de formato
     `10` quedan como formato anterior, sin migración (D7). Un valor grande que se comprime bien, como un texto,
     ocuparía su tamaño entero.
   - (b) Los registros grandes que, por una regla rápida y determinista, parecen no comprimirse (como T2, que fallaba
     con repeticiones largas). No cambia el formato: un registro sin comprimir ya es válido.
   - (c) Una variante de (b) sin muestras: sin comprimir los registros cuyo contenido son, sobre todo, páginas de
     valores fuera de su hoja. No cambia el formato.
   - (d) Las páginas de valores dentro de un flujo comprimido válido, guardadas como literales sin buscar
     repeticiones. No cambia el formato ni lo que lee la recuperación; está por ver si se puede hacer.
3. **¿Cuánto puede crecer un registro frente a hoy?** En bytes o en porcentaje, y si un registro que hoy se comprime
   31 veces puede guardarse entero.
4. **¿Puede fallar por disco lleno una confirmación que hoy cabe** porque su registro crece (J12, AC-J14), y crecer
   más deprisa el diario mientras una lectura larga detiene el traslado (J5)?
5. **¿Deben ser deterministas los bytes guardados?** Es decir, la misma confirmación da el mismo registro en cualquier
   máquina.

## Fuera de alcance

- Cambiar lo que contiene `krummi.data` o lo que leen las transacciones (segundo punto de J16).
- Leer o migrar diarios de formato `10` si se cambia el formato, y que el motor anterior lea diarios del nuevo.
- Cambiar el tamaño máximo de un registro, el límite de los archivos del diario o los umbrales de disco lleno (J5, J12).
- Una opción pública o una configuración para elegir.
- La compresión de las páginas hoja (5.6), el cifrado y otro algoritmo de compresión.
- Comprimir fuera del hilo que confirma: no cambia los bytes, pero la confirmación reserva su sitio en el archivo
  antes de codificar, y es otro cambio.

## Dejado a la implementación

A fijar cuando se respondan las preguntas.

## Criterios de aceptación

A fijar cuando se respondan las preguntas. Como mínimo: los registros que la opción elegida no toca son byte a byte
los de hoy, y la medición sigue la regla de la decisión del usuario del 2026-11-17 (ninguna carga empeora).

## Anotaciones a otros PRD

Pendientes de las respuestas: J16 (primer punto), Y1 (última frase) y Y6 (los archivos del diario se llenan más
despacio) se anotarían, y AC-J15 si sus páginas son de valores fuera de su hoja; D7 y Y5 si cambia el formato.

## Referencias

- [Plan de rendimiento](../../../performance/plan.md), *Writes after O5–O7* (perfil de W17).
- Ejecución de las optimizaciones de escritura: T2 (L2), la revisión R1 de Codex y su reversión (L11).
- Ejecución de las optimizaciones de la noche: L2 (por qué no hay regla exacta).
