# Experimento ANN del documento de diseño 12

- Fecha: 2026-11-24 (Windows 06:52–08:20 y 19:34–21:54 UTC; Linux 08:20–09:29 UTC)
- Ejecución: pasos 1, 2 y 3 de *Benchmarks* del
  [documento de diseño 12](../design/tables/12-vector-indexes.md), en memoria, con `krummi-ann-lab`
- Sistema: Windows 11 Pro 10.0.26200; Linux en WSL 2 (Ubuntu 24.04.1, núcleo 6.18.58.1-microsoft-standard-WSL2)
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Núcleos: en Windows, los 16 lógicos de rendimiento (afinidad `0xFFFF`); en WSL, sin fijar (el hipervisor reparte
  los procesadores virtuales)
- Reloj: en Windows, plan de energía del banco con el turbo apagado (los núcleos van a su frecuencia base), salvo
  en la medición de referencia con turbo; en WSL, el turbo lo maneja el anfitrión
- Memoria: 64 GiB (WSL: 31 GB)
- Krummi: `8984bc9` (barrido completo) y `d07655c` (barrido fino, HNSW y turbo)
- Rust: 1.99.0 en las dos plataformas
- Latencia: un hilo, caché caliente, 1000 consultas por configuración (100 para la búsqueda exacta)

## Resultado

**IVF-SQ8 cumple los umbrales en el caso de referencia, 1 M × 768 con L2.** Con nprobe 40 (4 % de las listas)
llega a recall@10 0.9578 con un p95 de 17.20 ms, a frecuencia base.

Dos criterios de *When the recommendation would change* se disparan, los dos por poco:

| Conjunto | Primer nprobe con recall@10 ≥ 0.95 | % de listas | p95 | Umbral (≤ 10 % y ≤ 20 ms) |
|---|---|---|---|---|
| Cohere 1 M × 768, L2 | 40 | 4.0 | 17.20 ms | cumple |
| Cohere 1 M × 768, producto escalar sin normalizar | 72 | 7.2 | 20.90 ms | no cumple el p95, por 0.9 ms |
| GloVe 1.19 M × 100, coseno | 160 | 14.7 | 6.18 ms | no cumple el % de listas |
| SIFT 1 M × 128, L2 | 32 | 3.2 | 2.49 ms | cumple |

El HNSW externo (paso 3) no resuelve los dos casos de la misma manera:

- **Producto escalar:** es unas 3.5 veces más rápido. Llega a 0.9608 con ef 80 y un p95 de 5.89 ms.
- **GloVe:** no llega a 0.95. Se queda en 0.9175 con ef 640, con M 16, el valor que se midió.
- **L2:** la ventaja es de 1.5 veces (0.9536 con un p95 de 11.19 ms), lejos de los 1–2 ms que estimaba el documento
  12.

Los núcleos de las distancias del documento 5 dan resultados idénticos bit a bit en Windows y en Linux (S4).

## Datos y método

| Conjunto | Filas | Dimensiones | Métrica | Consultas | Fuente |
|---|---|---|---|---|---|
| SIFT1M | 1 000 000 | 128 | L2 | 1000 propias del conjunto | `ftp.irisa.fr` |
| GloVe-100 | 1 192 514 | 100 | coseno | 1000 sacadas de la base con semilla 12 | `downloads.cs.stanford.edu` |
| Cohere Wikipedia 22-12 | 1 000 000 | 768 | L2 y producto escalar, sin normalizar (normas 13.5–14.4) | 1000 propias del conjunto | réplica pública de VectorDBBench |

- **Verdad de referencia:** el escaneo exacto de Krummi (top-100), con los núcleos del documento 5.
  - Contra la verdad de SIFT: recall@10 0.9994, y todas las diferencias son empates.
  - Contra la verdad de Cohere por coseno: 1.0000.
- **Escalas:** 10 000, 100 000 y todas las filas.
- **SQ8:** rangos por dimensión tomados de la muestra. Distancia asimétrica en `f32`, y reordenación exacta de
  `k·r` candidatos.
- **IVF:**
  - nlist según la regla de pgvector (filas / 1000);
  - k-means de 10 iteraciones con semilla fija, sobre una muestra de hasta 256 puntos por lista;
  - asignación y sondeo por L2 en el espacio de los códigos (normalizados para el coseno).
- **HNSW:** `hnsw_rs` 0.3.5, construido en paralelo.
  - M 16 (32 enlaces en la capa 0), ef_construction 200.
  - La distancia es la del núcleo del documento 5, desplazada para que nunca sea negativa.
  - Los candidatos se reordenan con la distancia exacta.
  - La búsqueda usa un ef efectivo de max(ef, k). Por eso, en las salidas sin editar, la columna recall@100 de
    ef 10 a 80 es la misma: corresponde a ef 100. La recall@10 de este informe sí usa el ef de cada fila.

## Cohere, 1 M × 768

Búsqueda exacta: p95 922 ms (L2) y 918 ms (producto escalar).

El escaneo SQ8 con reordenación da recall@10 0.9936 con r = 1, y 1.0000 desde r = 2. Su p95 es de 261.5 ms con r = 4.

Datos del índice IVF:

| | Valor |
|---|---|
| Listas | 1000 |
| Lista mayor | 3.96 veces la media |
| Construcción | 579–773 s en paralelo |
| IVF-SQ8 | 739 MiB |
| IVF-Flat | 2936 MiB |

IVF-SQ8 con r = 2 tiene la misma recall@10 que IVF-Flat en todas las filas, con un p95 unas 3.5 veces menor.

| Métrica | nprobe | % listas | recall@10 IVF-SQ8 (r = 2) | p50 ms | p95 ms | p99 ms |
|---|---|---|---|---|---|---|
| L2 | 32 | 3.2 | 0.9415 | 11.34 | 14.89 | 17.31 |
| L2 | **40** | 4.0 | **0.9578** | 13.34 | **17.20** | 19.11 |
| L2 | 48 | 4.8 | 0.9658 | 15.63 | 20.30 | 21.77 |
| L2 | 64 | 6.4 | 0.9773 | 22.01 | 27.99 | 30.78 |
| Producto escalar | 64 | 6.4 | 0.9483 | 15.38 | 19.65 | 21.37 |
| Producto escalar | **72** | 7.2 | **0.9541** | 16.44 | **20.90** | 22.61 |
| Producto escalar | 80 | 8.0 | 0.9594 | 18.04 | 22.42 | 24.11 |
| Producto escalar | 128 | 12.8 | 0.9750 | 29.97 | 35.45 | 38.35 |

HNSW externo:

| | L2 | Producto escalar |
|---|---|---|
| Construcción, en paralelo | 1169 s | 1791 s |
| Enlaces de la capa 0, estimación compacta (4 bytes por enlace) | 122 MiB | 122 MiB |
| Vectores `f32` | 2930 MiB | 2930 MiB |

La estimación compacta no es lo que ocupa `hnsw_rs`, que guarda punteros compartidos. Su memoria medida está en
*Memoria*.

| ef | recall@10 L2 | p95 L2 ms | recall@10 producto escalar | p95 producto escalar ms |
|---|---|---|---|---|
| 40 | 0.8668 | 2.24 | 0.9281 | 3.32 |
| 80 | 0.9090 | 3.71 | **0.9608** | **5.89** |
| 160 | 0.9367 | 6.36 | 0.9811 | 9.65 |
| 320 | **0.9536** | **11.19** | 0.9912 | 16.31 |
| 640 | 0.9632 | 20.74 | 0.9950 | 27.95 |

## GloVe, 1.19 M × 100, coseno

Búsqueda exacta: p95 148 ms.

Datos del índice IVF: 1092 listas, la mayor 2.73 veces la media; construcción en 89–128 s; IVF-SQ8 119 MiB.

| nprobe | % listas | recall@10 IVF-SQ8 (r = 2) | p95 ms |
|---|---|---|---|
| 64 | 5.9 | 0.9003 | 3.29 |
| 128 | 11.7 | 0.9457 | 5.30 |
| **160** | **14.7** | **0.9572** | **6.18** |
| 192 | 17.6 | 0.9666 | 7.33 |
| 256 | 23.4 | 0.9792 | 9.69 |

HNSW externo: construcción en 1626 s, con 146 MiB de enlaces y 455 MiB de vectores.

| ef | 80 | 160 | 320 | 640 |
|---|---|---|---|---|
| recall@10 | 0.7721 | 0.8387 | 0.8826 | 0.9175 |
| p95 ms | 4.28 | 6.63 | 11.81 | 19.24 |

GloVe es difícil para los dos algoritmos, como anticipaba el documento 12:

- IVF necesita sondear más listas, pero su latencia queda lejos de los 20 ms.
- El grafo con M 16 no llega a 0.95.

## SIFT, 1 M × 128, L2

- Búsqueda exacta: p95 160 ms.
- IVF-SQ8 con nprobe 32 (3.2 %): recall@10 0.9762, con un p95 de 2.49 ms.
- Construcción del índice: 112 s.

## Escalas menores

| Conjunto | Filas | Primer nprobe con recall@10 ≥ 0.95 | % de listas | p95 |
|---|---|---|---|---|
| SIFT | 100 000 | 8 | 8.0 | 0.60 ms |
| GloVe | 100 000 | 32 | 32.0 | 1.32 ms |
| Cohere L2 | 100 000 | 16 | 16.0 | 7.14 ms |
| Cohere, producto escalar | 100 000 | 32 | 32.0 | 8.83 ms |

Con 10 000 filas hay 10 listas, y todas las configuraciones necesitan sondear del 40 al 80 % de ellas.

A estas escalas, la búsqueda exacta ya tarda menos de 20 ms hasta las 100 000 filas de 128 dimensiones. Con 768
dimensiones, eso vale hasta unas 20 000 filas (extrapolado de las 10 000, que tardan 9.22 ms).

## Windows y Linux

Los 12 resúmenes (FNV-1a de los ids y las distancias de todas las consultas) son idénticos en las dos plataformas. Por
ejemplo, `9179f4f2569a6b41` para Cohere L2 a 1 M.

Linux, sin fijar núcleos y con el turbo del anfitrión, es más rápido. Con Cohere L2 a 1 M:

- búsqueda exacta: p95 554.6 ms, contra 922.1 ms en Windows;
- nprobe 64: p95 21.62 ms, contra 27.99 ms.

## El reloj

El plan del banco apaga el turbo, así que todos los p95 de Windows son a frecuencia base. Con turbo, las dos pasadas
completas se aceleran entre 1.7 y 1.8 veces en los conjuntos de 10 000 filas:

| Conjunto | Exacta sin turbo | Exacta con turbo | Escaneo SQ8 sin turbo | Escaneo SQ8 con turbo |
|---|---|---|---|---|
| SIFT, 10 000 | 1.46 ms | 0.86 ms | 0.50 ms | 0.30 ms |
| Cohere L2, 10 000 | 9.22 ms | 5.19 ms | 2.60 ms | 1.46 ms |

No se midió con turbo a 1 M. Por eso el p95 del producto escalar con nprobe 72 (20.90 ms) **no se da por cumplido**.

## Memoria

El RSS se midió en Linux, a 100 000 filas, leyendo `VmRSS` de `/proc/self/status` antes y después de construir cada
estructura. Windows no tiene una API segura en la biblioteca estándar, y el workspace prohíbe `unsafe`.

El crecimiento incluye los temporales de la construcción que el asignador no devolvió. En un mismo proceso, el
asignador reutiliza lo que liberó una sección anterior, y el crecimiento sale más bajo: el producto escalar de Cohere
medido justo después de L2 dio +0.0 MiB para los códigos SQ8. Por eso cada cifra de esta tabla es la primera sección
de su proceso.

| Conjunto, 100 000 filas | Estructura | Tamaño calculado | Crecimiento del RSS |
|---|---|---|---|
| SIFT, 128 | Códigos SQ8 | 12.2 MiB | 25.4 MiB |
| SIFT, 128 | IVF-SQ8 e IVF-Flat juntos | 61.9 MiB | 138.3 MiB |
| GloVe, 100 | Códigos SQ8 | 9.5 MiB | 20.0 MiB |
| GloVe, 100 | IVF-SQ8 e IVF-Flat juntos | 48.6 MiB | 105.8 MiB |
| GloVe, 100 | HNSW (`hnsw_rs`) | 50.3 MiB, estimación compacta | 302.4 MiB |
| Cohere L2, 768 | Códigos SQ8 | 73.2 MiB | 149.2 MiB |
| Cohere L2, 768 | IVF-SQ8 e IVF-Flat juntos | 367.5 MiB | 792.2 MiB |
| Cohere L2, 768 | HNSW (`hnsw_rs`) | 305.2 MiB, estimación compacta | 460.3 MiB |
| Cohere, producto escalar, 768 | IVF-SQ8 e IVF-Flat juntos | 367.5 MiB | 790.0 MiB |
| Cohere, producto escalar, 768 | HNSW (`hnsw_rs`) | 305.2 MiB, estimación compacta | 566.0 MiB |

Estas cifras dan una idea de lo que ocupa cada estructura:

- **El grafo de `hnsw_rs`, sin contar los vectores,** ocupa unos 2.7–2.8 KiB por vector con GloVe y con el producto
  escalar de Cohere: 302.4 − 38.1 MiB de vectores y 566.0 − 293.0 MiB, en 100 000 filas. La estimación compacta es de
  128 bytes por vector. Con L2, el mismo cálculo da unos 1.7 KiB.
- **Extrapolado a 1 M:** unos 2.6 GiB de grafo más los vectores. Esto no se midió.
- **IVF:** el crecimiento es algo más del doble del tamaño calculado, porque la construcción copia la base a un espacio
  transformado y la libera al terminar. El índice que queda es el del tamaño calculado.
- **El pico del proceso** (`VmHWM`) lo fija la carga de la base completa, antes de recortarla a 100 000 filas: 5.7 GiB
  con Cohere. No mide ninguna estructura.

## Lo que no se midió

- **Memoria en Windows y a 1 M:** el RSS solo se midió en Linux y a 100 000 filas (*Memoria*).
- **Caché fría y conjunto de trabajo en la caché de páginas:** no aplican en memoria. Quedan para el índice
  almacenado.
- **10 M, filtros, deriva, corrección, durabilidad y costo operativo:** son mediciones del índice almacenado, de 7.6.
- **HNSW en Linux:** su construcción en paralelo no es determinista, así que no tiene resumen que comparar entre
  plataformas.
- **Otros parámetros del HNSW:** M 32 o un ef_construction mayor no se midieron.

## Archivos

Salidas sin editar de `krummi-ann-lab`:

- [Windows, barrido completo](2026-11-24-ann-experiment/windows.md)
- [Linux, barrido completo](2026-11-24-ann-experiment/linux.md)
- [Windows, barrido fino](2026-11-24-ann-experiment/windows-fine.md)
- [Windows, HNSW](2026-11-24-ann-experiment/windows-hnsw.md)
- [Windows con turbo](2026-11-24-ann-experiment/windows-boost.md)
- [Linux, memoria a 100 000 filas](2026-11-24-ann-experiment/linux-memory.md)
- [Linux, memoria del producto escalar](2026-11-24-ann-experiment/linux-memory-dot.md)
