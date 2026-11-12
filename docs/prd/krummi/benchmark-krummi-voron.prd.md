# Banco solo con Krummi y Voron, en los núcleos de rendimiento — PRD

**Versión**: 1.0
**Fecha**: 2026-11-12
**Documento padre**: [Banco comparativo de Krummi — PRD](benchmark.prd.md) (v1.0)

## Objetivo y contexto

Dos cambios en cómo se ejecuta el banco del módulo 6, sin cambiar sus cargas, su medición ni sus umbrales:

1. **Un modo opcional que mide solo Krummi y Voron.** Una ejecución completa con los seis motores tarda unas 4 horas
   en el equipo del mantenedor, y casi todo ese tiempo se lo llevan redb, canopydb y fjall: en el primer intento de
   ejecución completa del 2026-11-11, el banco había compilado Voron y medido Krummi, Voron y LMDB unos 44 minutos
   después de empezar. Los umbrales (K7) solo comparan Krummi con Voron, así que para saber si Krummi los cumple
   bastan esos dos motores. Pedido del usuario: «El banco tarda demasiado, hagamos opcional un banco que compare solo
   Krummi y Voron.»
2. **Medir en los núcleos de rendimiento, en toda ejecución.** En un procesador con núcleos de rendimiento y de
   eficiencia, Windows puede mover un proceso de medición a los de eficiencia, y su resultado cae a la mitad o a un
   cuarto: en las mediciones del 2026-11-11 y del 2026-11-12, una tanda de Voron salió a la mitad de sus cifras
   habituales mientras la de Krummi era normal, y limitar los procesos a los núcleos de rendimiento devolvió las cifras
   de siempre. Una razón Krummi / Voron calculada con uno de los dos motores en núcleos de eficiencia no mide los
   motores, sino el reparto del sistema. Este cambio vale con y sin el modo nuevo, y añade una línea a todos los
   informes. A meterlo en el mismo PRD, el usuario respondió: «Mételo».

Este PRD hereda todo lo del [banco](benchmark.prd.md) y de su padre, el [PRD raíz](krummi.prd.md). Aquí solo se añade
lo propio de estos dos cambios; lo que cambia o precisa reglas del padre está declarado en sus decisiones y anotado
en el padre (*Anotaciones a otros PRD*).

### Decisiones de producto

| ID | Decisión |
|---|---|
| B1 | **La opción `--solo-krummi-voron`** (nombre elegido por el usuario) añade a la orden de K10 un modo que mide solo Krummi y Voron: `cargo run --release -p krummi-bench -- --ravendb <carpeta> [--carpeta <carpeta>] [--informe <archivo>] [--reducido] [--solo-krummi-voron]`. Como `--reducido`, no lleva valor; se combina con todas las demás opciones. |
| B2 | **Qué mide el modo**: lo mismo que una ejecución de los seis motores, salvo que LMDB, redb, canopydb y fjall no se preparan ni se ejecutan: Krummi y después Voron (el orden de K3 sin los otros cuatro), las 29 cargas con sus cantidades, repeticiones y datos, y los 29 umbrales. |
| B3 | **El informe del modo** tiene la estructura de *Salida* del banco con solo dos columnas de motores, Krummi y Voron, en las dos tablas, seguidas en la de operaciones por *Krummi / Voron* y *Umbral*; tiene las 29 filas de operaciones y las de los tamaños de K9, sin ninguna celda `no aplica` (los dos motores ofrecen todas las estructuras, K6). La línea *Motores* nombra solo Voron con su versión de .NET; la línea *Ejecución* dice `completa, solo Krummi y Voron` o `reducida, solo Krummi y Voron`; *Opciones cambiadas* solo tiene líneas de esos dos motores (con las opciones del banco, `Ninguna.`). La salida estándar, el código de salida y las reglas del informe reducido (`1 medida`, `sin evaluar`) son los del banco. |
| B4 | **Núcleos de rendimiento**: en toda ejecución del banco, con o sin `--solo-krummi-voron`, completa o reducida, los procesos que el banco lanza para un motor (los que preparan sus almacenes y los que miden sus cargas), y todos los hilos y procesos que estos lancen, corren solo en los núcleos lógicos de rendimiento cuando el sistema operativo los distingue. Los distingue cuando da a sus núcleos lógicos más de una clase de eficiencia o de capacidad; los de rendimiento son los de la clase más rápida. Nada más cuenta como clase: ni la frecuencia de cada núcleo ni los núcleos que el sistema prefiere. El número de hilos de cada carga no cambia (K11), aunque sea mayor que el de esos núcleos. Cuando el sistema da a todos sus núcleos la misma clase, o no da ninguna (como WSL 2 o una máquina virtual que no la expone), los procesos no se limitan. Si el banco no consigue preguntar al sistema por las clases, o no consigue limitar un proceso, la ejecución sigue: ese proceso, o todos si no se pudo preguntar, miden sin limitar (decisión del usuario: «Medir sin fijar»). El resto del banco, como compilar Voron o escribir el informe, no se limita. |
| B5 | **El informe dice dónde midió**: una línea nueva, *Núcleos*, justo después de *Procesador*, con o sin `--solo-krummi-voron`, también cuando *Procesador* dice `desconocido`. Dice `Núcleos: <n> lógicos de rendimiento de <total> (los motores se miden en ellos)` cuando todos los procesos de los motores se limitaron (B4), con `<n>` los núcleos lógicos de rendimiento y `<total>` todos los núcleos lógicos que el sistema da al equipo; `Núcleos: sin fijar (el sistema no distingue núcleos de rendimiento)` cuando el sistema da a todos sus núcleos la misma clase o no da ninguna; y `Núcleos: desconocido` cuando el banco no consiguió preguntar por las clases o no consiguió limitar algún proceso, como cualquier dato del equipo que no puede obtener (*Salida* del banco). El banco no escribe un mensaje propio por ello: la salida de errores solo lleva su progreso, con el formato que el banco deja a la implementación. |
| B6 | **AC19 y las ejecuciones previas a una versión**: una ejecución completa con `--solo-krummi-voron` en Windows y en Linux vale para comprobar los umbrales de AC19 del PRD raíz, porque los umbrales solo comparan Krummi con Voron (decisión del usuario: «Sí, vale»). La comparación con los demás motores solo sale de una ejecución de los seis. |

### Estado de partida

Verificado el 2026-11-12 en el repositorio:

| Aspecto | Situación |
|---|---|
| Orden del banco | Acepta `--ravendb`, `--carpeta`, `--informe` y `--reducido`; cualquier otro argumento da `Opción no válida: <opción>.` con código `2`. `--reducido` seguido de un argumento suelto da el error con ese argumento. |
| Motores | El banco mide siempre los seis, en el orden de K3, y el informe tiene siempre seis columnas de motores. El padre deja fuera de alcance «Elegir desde la orden qué motores o qué cargas ejecutar». |
| Núcleos | El banco no limita los núcleos de ningún proceso, y el padre deja la preparación del equipo («antivirus, modo de energía, procesos en segundo plano») al mantenedor. Las mediciones de las tablas del 2026-11-11 y del 2026-11-12 limitaron los procesos a mano a los núcleos de rendimiento del equipo del mantenedor (un i9-13980HX: 16 núcleos lógicos de rendimiento de 32). |
| Duración | Ejecución completa de los seis motores en Windows: unas 4 horas; la compilación de Voron y las cargas de Krummi, Voron y LMDB, unos 44 minutos. |
| Integración continua | Ejecuta el banco con `--reducido` y los seis motores en Windows y en Linux (K12). |

## Actores

- **Mantenedor**: ejecuta el banco con `--solo-krummi-voron` para comprobar los umbrales tras un cambio o antes de una
  versión (B6), y sin ella cuando quiere la comparación con todos los motores.
- **Integración continua**: sin cambios de orden (K12); su informe gana la línea *Núcleos*.

## Flujo

El del banco. Antes de medir, el banco averigua qué núcleos son de rendimiento (B4); con `--solo-krummi-voron`, solo mide
Krummi y Voron (B2).

## Validaciones y mensajes

Las del banco, con `--solo-krummi-voron` como una opción más de su tabla, sin valor como `--reducido`: repetida, el error
nombra su segunda aparición; escrita como `--solo-krummi-voron=<algo>`, nombra ese argumento entero; y seguida de un argumento
que no es una opción, nombra ese argumento. Con `--solo-krummi-voron` siguen valiendo los requisitos de RavenDB y del SDK de
.NET 10, porque Voron se mide.

## Salida

La salida estándar no cambia. Ejemplo de informe con `--solo-krummi-voron` (cifras ilustrativas, solo algunas filas):

```markdown
# Banco comparativo de Krummi

- Fecha: 2026-11-12 16:00 (UTC+00:00)
- Ejecución: completa, solo Krummi y Voron
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: 0123456789ab
- Motores: Voron 7.2.7 (.NET 10.0.5)
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 29 de 29 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 943.198 | 666.145 | 1,41 | cumple |
| tabla-actualizacion | 114.865 | 129.932 | 0,88 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron |
|---|---|---|
| carga-en-orden | 150.994.944 | 160.432.128 |

## Opciones cambiadas

Ninguna.
```

Una ejecución sin `--solo-krummi-voron` tiene el informe de siempre, con la línea *Núcleos* añadida.

## Reutilización

- La orden, sus validaciones, la medición, el informe y sus reglas de formato del banco: el modo solo deja fuera cuatro
  motores.
- Los procesos que ya lanzan cada carga (K3): B4 los limita a los núcleos de rendimiento.

## Fuera de alcance

- Elegir otros motores o cargas que el par Krummi y Voron: el resto de esa exclusión del padre se mantiene.
- Hacer `--solo-krummi-voron` el modo predeterminado, o darle alias.
- Elegir a mano los núcleos, la prioridad de los procesos, un núcleo lógico por núcleo físico, o fijar cada hilo a un
  núcleo concreto.
- Ajustar el número de hilos de una carga al de núcleos de rendimiento.
- Limitar los procesos ajenos al banco o el resto del banco; configurar el modo de energía, que sigue siendo
  responsabilidad del mantenedor.
- Distinguir núcleos donde el sistema operativo no los distingue: en una máquina virtual, y en WSL 2 cuando no los
  expone, los procesos no se limitan y las mediciones de Linux siguen expuestas al reparto del sistema (decisión del
  usuario: «Aceptarlo»); indicar a mano qué núcleos usar.
- Comprobar después de medir que un proceso no salió de sus núcleos, o medir su uso de CPU.
- Volver a medir o reetiquetar los informes anteriores: se distinguen de los nuevos porque no tienen la línea
  *Núcleos*.
- Cambiar la integración continua (K12), reanudar una ejecución o combinar informes de un modo y del otro.

## Dejado a la implementación

- Cómo se averiguan los núcleos de rendimiento en cada sistema y cómo se limita un proceso a ellos.

## Criterios de aceptación

1. **AC-B1** — Dada una ejecución completa con `--solo-krummi-voron` en Windows y otra en Linux, cuando terminan, entonces
   cada una escribe un informe con la estructura de B3, las 29 filas de operaciones, las 12 de tamaños (K9 con su
   anotación de 7.1), ninguna celda `no aplica` y la línea *Núcleos* de B5; y, al observar sus procesos, solo midieron
   Krummi y después Voron.
2. **AC-B2** — Dados unos resultados por carga y motor y unos datos del equipo inyectados para la prueba, cuando el
   banco escribe el informe con y sin `--solo-krummi-voron`, completo y reducido, con los umbrales cumplidos y con uno
   incumplido, con núcleos de rendimiento, sin fijar y desconocidos, y con *Procesador* desconocido, entonces el informe
   y la línea de la salida estándar son idénticos byte a byte a los esperados, y el código de salida es el del banco.
3. **AC-B3** — Dadas unas clases de núcleos inyectadas para la prueba (todas iguales, ninguna, dos y tres clases),
   cuando el banco decide dónde medir, entonces elige los núcleos lógicos de la clase más rápida cuando hay más de una,
   y ninguno en los otros casos, con la línea *Núcleos* de B5 de cada caso. Dado un procesador con núcleos de
   rendimiento que el sistema distingue, cuando el banco lanza los procesos de un motor, entonces la afinidad que el
   sistema da a cada uno, y a un proceso que este lance, son esos núcleos, y la del proceso del banco no cambia. Dado
   un fallo inyectado al preguntar por las clases, o al limitar uno solo de los procesos, con los mismos resultados por
   carga y motor y los mismos datos del equipo inyectados en la ejecución con el fallo y en la ejecución sin él,
   entonces las dos terminan con el mismo código y el mismo informe, salvo que la primera dice `Núcleos: desconocido`;
   en una ejecución real, los umbrales y el código de salida siguen saliendo de lo que se midió (K7, K13).
4. **AC-B4** — Dada `--solo-krummi-voron` repetida, escrita con `=` o seguida de un argumento suelto, cuando se ejecuta la
   orden, entonces muestra `Opción no válida: <opción>.` con la `<opción>` de *Validaciones y mensajes*, termina con
   código `2` y no hace nada. Con `--solo-krummi-voron`, AC-K5 y AC-K9 del banco siguen valiendo.

## Anotaciones a otros PRD

Se añaden al [banco](benchmark.prd.md), al [PRD raíz](krummi.prd.md) y a sus traducciones:

- [benchmark.prd.md](benchmark.prd.md), K10 y *Validaciones y mensajes*: *«Anotación del 2026-11-12 desde el banco
  solo con Krummi y Voron ([benchmark-krummi-voron.prd.md](benchmark-krummi-voron.prd.md)): la orden admite además
  `--solo-krummi-voron`, sin valor, que mide solo Krummi y Voron (B1, B2); la tabla de validaciones cuenta cinco opciones.»*
- [benchmark.prd.md](benchmark.prd.md), K3 y AC-K11: *«Anotación del 2026-11-12 desde el banco solo con Krummi y
  Voron: con `--solo-krummi-voron`, el banco mide solo Krummi y después Voron (B2); en toda ejecución, cada proceso que mide
  una carga se limita a los núcleos de rendimiento cuando el sistema los distingue (B4).»*
- [benchmark.prd.md](benchmark.prd.md), *Salida*, AC-K1 y AC-K2: *«Anotación del 2026-11-12 desde el banco solo con
  Krummi y Voron: todo informe lleva la línea *Núcleos* después de *Procesador* (B5), también en los textos esperados
  de AC-K2; excepción a «Las columnas de motores son siempre las seis»: con `--solo-krummi-voron`, el informe tiene
  solo las columnas de Krummi y Voron (B3); AC-K1 vale para las ejecuciones sin esa opción.»*
- [benchmark.prd.md](benchmark.prd.md), *Actores* y *Flujo*: *«Anotación del 2026-11-12 desde el banco solo con Krummi
  y Voron: antes de cada versión, el mantenedor puede ejecutar el banco con `--solo-krummi-voron` (B6); el banco averigua
  los núcleos de rendimiento antes de medir, y con esa opción solo mide Krummi y Voron (B2, B4).»*
- [benchmark.prd.md](benchmark.prd.md), *Fuera de alcance*: *«Anotación del 2026-11-12 desde el banco solo con Krummi
  y Voron: excepciones a «Elegir desde la orden qué motores o qué cargas ejecutar», porque `--solo-krummi-voron` elige el par
  Krummi y Voron (B1), y a «Preparar el equipo», porque el banco limita sus procesos de medición a los núcleos de
  rendimiento (B4).»*
- [krummi.prd.md](krummi.prd.md), AC19: *«Anotación del 2026-11-12 desde el banco solo con Krummi y Voron: una
  ejecución completa con `--solo-krummi-voron` en Windows y en Linux, con solo Krummi y Voron, vale para comprobar los
  umbrales (B6).»*

## Referencias

- Padre: [Banco comparativo de Krummi — PRD](benchmark.prd.md) (K3, K6, K7, K10, K11, K12, K13, *Validaciones y
  mensajes*, *Salida*, *Fuera de alcance*, AC-K1, AC-K2, AC-K5, AC-K9, AC-K11).
- Raíz: [Krummi — PRD](krummi.prd.md) (*Salida*, AC19).
- Mediciones que motivan B4: L13, L31 y L40 de [odd/tasks/table-row-reads.md](../../../odd/tasks/table-row-reads.md).
