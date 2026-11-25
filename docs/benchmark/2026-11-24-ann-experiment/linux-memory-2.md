# Experimento ANN: Linux (WSL), memoria en procesos propios, 100 000 filas y Cohere L2 a 1 M (código de fe18410)

Salida de `krummi-ann-lab`, sin editar. Informe: [../2026-11-24-ann-experiment.md](../2026-11-24-ann-experiment.md).

### sift, 100000 filas de 128 dimensiones, l2

Búsqueda exacta (100 consultas): p50 8.98 ms, p95 10.43 ms, p99 10.71 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 12.2 MiB en 0.1 s; 265 códigos con algún componente saturado (371 componentes); el RSS creció 25.4 MiB al construirlo (de 54.7 a 80.1 MiB, pico del proceso 983.0 MiB; incluye los temporales que el asignador no devolvió).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9913 | 0.9940 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 2.60 ms, p95 3.64 ms, p99 4.01 ms.

Paso 2, IVF: 100 listas, muestra de 25600 vectores, construido en 2.4 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 2.12 veces la media; carga útil (longitudes de ids, códigos o vectores y centroides): IVF-SQ8 12.6 MiB, IVF-Flat 49.3 MiB; 265 códigos con algún componente saturado; los dos índices juntos tienen asignados 90.2 MiB (capacidad de cada búfer, normas incluidas), y el RSS creció 138.6 MiB al construirlo (de 89.0 a 227.6 MiB, pico del proceso 983.0 MiB; incluye los temporales que el asignador no devolvió).

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 8 | 8.0 | IVF-Flat | — | 0.9641 | 0.9273 | 0.98 | 1.57 | 1.86 | 965 |
| 8 | 8.0 | IVF-SQ8 | 2 | 0.9641 | — | 0.35 | 0.60 | 0.69 | 2679 |
| 8 | 8.0 | IVF-SQ8 | 4 | 0.9641 | 0.9273 | 0.25 | 0.34 | 0.38 | 3954 |
| 8 | 8.0 | IVF-SQ8 | 8 | 0.9641 | — | 0.35 | 0.49 | 0.57 | 2809 |

Resumen de los resultados (FNV-1a de ids y distancias): `58e23e1eb14df1f6`.

### glove, 100000 filas de 100 dimensiones, coseno

Búsqueda exacta (100 consultas): p50 6.26 ms, p95 6.86 ms, p99 7.30 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 9.5 MiB en 0.0 s; 482 códigos con algún componente saturado (482 componentes); el RSS creció 20.0 MiB al construirlo (de 43.9 a 63.8 MiB, pico del proceso 916.9 MiB; incluye los temporales que el asignador no devolvió).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9842 | 0.9880 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 1.81 ms, p95 2.23 ms, p99 2.42 ms.

Paso 2, IVF: 100 listas, muestra de 25600 vectores, construido en 0.7 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 2.24 veces la media; carga útil (longitudes de ids, códigos o vectores y centroides): IVF-SQ8 10.0 MiB, IVF-Flat 38.6 MiB; 482 códigos con algún componente saturado; los dos índices juntos tienen asignados 69.7 MiB (capacidad de cada búfer, normas incluidas), y el RSS creció 106.7 MiB al construirlo (de 73.0 a 179.8 MiB, pico del proceso 916.9 MiB; incluye los temporales que el asignador no devolvió).

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 8 | 8.0 | IVF-Flat | — | 0.8277 | 0.7495 | 0.56 | 0.83 | 0.94 | 1727 |
| 8 | 8.0 | IVF-SQ8 | 2 | 0.8277 | — | 0.23 | 0.36 | 0.42 | 4113 |
| 8 | 8.0 | IVF-SQ8 | 4 | 0.8277 | 0.7495 | 0.17 | 0.24 | 0.31 | 5693 |
| 8 | 8.0 | IVF-SQ8 | 8 | 0.8277 | — | 0.23 | 0.34 | 0.42 | 4166 |

Resumen de los resultados (FNV-1a de ids y distancias): `7474935806ac7dbe`.

### cohere, 100000 filas de 768 dimensiones, l2

Búsqueda exacta (100 consultas): p50 46.09 ms, p95 55.27 ms, p99 56.83 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 73.2 MiB en 0.4 s; 3516 códigos con algún componente saturado (4057 componentes); el RSS creció 149.2 MiB al construirlo (de 301.2 a 450.4 MiB, pico del proceso 5865.5 MiB; incluye los temporales que el asignador no devolvió).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9942 | 0.9941 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 13.45 ms, p95 15.79 ms, p99 16.63 ms.

Paso 2, IVF: 100 listas, muestra de 25600 vectores, construido en 5.3 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 3.54 veces la media; carga útil (longitudes de ids, códigos o vectores y centroides): IVF-SQ8 73.9 MiB, IVF-Flat 293.6 MiB; 3516 códigos con algún componente saturado; los dos índices juntos tienen asignados 531.5 MiB (capacidad de cada búfer, normas incluidas), y el RSS creció 787.8 MiB al construirlo (de 459.0 a 1246.8 MiB, pico del proceso 5865.5 MiB; incluye los temporales que el asignador no devolvió).

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 8 | 8.0 | IVF-Flat | — | 0.9001 | 0.8113 | 5.40 | 8.94 | 10.67 | 176 |
| 8 | 8.0 | IVF-SQ8 | 2 | 0.9001 | — | 1.53 | 2.55 | 3.46 | 616 |
| 8 | 8.0 | IVF-SQ8 | 4 | 0.9001 | 0.8113 | 1.54 | 2.39 | 2.75 | 624 |
| 8 | 8.0 | IVF-SQ8 | 8 | 0.9001 | — | 1.63 | 2.96 | 4.04 | 566 |

Resumen de los resultados (FNV-1a de ids y distancias): `d8c78c7bc7278d65`.

### cohere, 100000 filas de 768 dimensiones, dot

Búsqueda exacta (100 consultas): p50 47.54 ms, p95 51.77 ms, p99 58.98 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 73.2 MiB en 0.4 s; 3516 códigos con algún componente saturado (4057 componentes); el RSS creció 149.2 MiB al construirlo (de 301.2 a 450.4 MiB, pico del proceso 5865.4 MiB; incluye los temporales que el asignador no devolvió).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9911 | 0.9932 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 10.31 ms, p95 11.72 ms, p99 12.34 ms.

Paso 2, IVF: 100 listas, muestra de 25600 vectores, construido en 4.5 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 3.54 veces la media; carga útil (longitudes de ids, códigos o vectores y centroides): IVF-SQ8 73.9 MiB, IVF-Flat 293.6 MiB; 3516 códigos con algún componente saturado; los dos índices juntos tienen asignados 531.5 MiB (capacidad de cada búfer, normas incluidas), y el RSS creció 787.6 MiB al construirlo (de 459.5 a 1247.0 MiB, pico del proceso 5865.4 MiB; incluye los temporales que el asignador no devolvió).

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 8 | 8.0 | IVF-Flat | — | 0.8767 | 0.8217 | 5.15 | 7.91 | 8.76 | 190 |
| 8 | 8.0 | IVF-SQ8 | 2 | 0.8767 | — | 1.02 | 1.65 | 1.99 | 930 |
| 8 | 8.0 | IVF-SQ8 | 4 | 0.8767 | 0.8217 | 1.01 | 1.52 | 1.82 | 956 |
| 8 | 8.0 | IVF-SQ8 | 8 | 0.8767 | — | 1.10 | 1.74 | 2.35 | 862 |

Resumen de los resultados (FNV-1a de ids y distancias): `2affdede1b332462`.

### cohere, 1000000 filas de 768 dimensiones, l2

Búsqueda exacta (100 consultas): p50 444.75 ms, p95 496.90 ms, p99 505.80 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 732.4 MiB en 3.6 s; 4127 códigos con algún componente saturado (4270 componentes); el RSS creció 1486.3 MiB al construirlo (de 2951.7 a 4438.0 MiB, pico del proceso 5865.4 MiB; incluye los temporales que el asignador no devolvió).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9936 | 0.9935 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 126.28 ms, p95 142.34 ms, p99 150.00 ms.

Paso 2, IVF: 1000 listas, muestra de 256000 vectores, construido en 473.5 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 3.96 veces la media; carga útil (longitudes de ids, códigos o vectores y centroides): IVF-SQ8 739.2 MiB, IVF-Flat 2936.4 MiB; 4127 códigos con algún componente saturado; los dos índices juntos tienen asignados 5252.2 MiB (capacidad de cada búfer, normas incluidas), y el RSS creció 7695.9 MiB al construirlo (de 4446.4 a 12142.3 MiB, pico del proceso 12142.3 MiB; incluye los temporales que el asignador no devolvió).

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 40 | 4.0 | IVF-Flat | — | 0.9578 | 0.9169 | 23.59 | 31.32 | 35.73 | 41 |
| 40 | 4.0 | IVF-SQ8 | 2 | 0.9578 | — | 7.34 | 9.98 | 11.74 | 133 |
| 40 | 4.0 | IVF-SQ8 | 4 | 0.9578 | 0.9169 | 7.09 | 9.37 | 10.46 | 138 |
| 40 | 4.0 | IVF-SQ8 | 8 | 0.9578 | — | 7.75 | 10.35 | 11.23 | 125 |

Resumen de los resultados (FNV-1a de ids y distancias): `58b7619f3c2737d5`.

### cohere, 1000000 filas de 768 dimensiones, l2

Paso 3, HNSW externo (`hnsw_rs`): M 16 (2·M en la capa 0), ef_construction 200, construido en 1189.6 s en todos los núcleos; con identificadores de 4 bytes, los enlaces de la capa 0 ocuparían 122.1 MiB (2·M por vector) y los vectores 2929.7 MiB, una estimación compacta: `hnsw_rs` guarda punteros compartidos y su propia copia de los vectores; el grafo: el RSS creció 4618.3 MiB al construirlo (de 5911.5 a 10529.8 MiB, pico del proceso 10529.8 MiB; incluye los temporales que el asignador no devolvió). Sin resumen: la construcción en paralelo no es determinista.

| ef | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|
| 10 | 0.6774 | 0.8696 | 1.23 | 2.48 | 3.29 | 735 |
| 20 | 0.7972 | 0.8696 | 1.09 | 1.83 | 2.30 | 880 |
| 40 | 0.8665 | 0.8696 | 1.31 | 1.99 | 2.51 | 744 |
| 80 | 0.9070 | 0.8696 | 1.83 | 2.61 | 3.03 | 540 |
| 160 | 0.9352 | 0.9111 | 3.00 | 4.06 | 4.53 | 331 |
| 320 | 0.9514 | 0.9477 | 5.46 | 7.18 | 7.93 | 184 |
| 640 | 0.9603 | 0.9670 | 9.89 | 13.13 | 14.59 | 101 |

