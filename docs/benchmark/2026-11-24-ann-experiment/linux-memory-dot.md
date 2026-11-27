# Experimento ANN: Linux (WSL), memoria del producto escalar en procesos propios (código de 96ee454)

Salida de `krummi-ann-lab`, sin editar. Informe: [../2026-11-24-ann-experiment.md](../2026-11-24-ann-experiment.md).

### cohere, 100000 filas de 768 dimensiones, dot

Búsqueda exacta (100 consultas): p50 61.42 ms, p95 71.36 ms, p99 75.44 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 73.2 MiB en 0.4 s; 3516 códigos con algún componente saturado (4057 componentes); el RSS creció 149.2 MiB al construirlo (de 301.2 a 450.4 MiB, pico del proceso 5865.5 MiB; incluye los temporales que el asignador no devolvió).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9911 | 0.9932 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 10.80 ms, p95 12.78 ms, p99 13.82 ms.

Paso 2, IVF: 100 listas, muestra de 25600 vectores, construido en 6.5 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 3.54 veces la media; IVF-SQ8 73.9 MiB, IVF-Flat 293.6 MiB; 3516 códigos con algún componente saturado; los dos índices juntos: el RSS creció 790.0 MiB al construirlo (de 458.9 a 1249.0 MiB, pico del proceso 5865.5 MiB; incluye los temporales que el asignador no devolvió).

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.0 | IVF-Flat | — | 0.4973 | 0.4198 | 0.66 | 1.25 | 1.80 | 1359 |
| 1 | 1.0 | IVF-SQ8 | 2 | 0.4973 | — | 0.31 | 0.63 | 0.91 | 2915 |
| 1 | 1.0 | IVF-SQ8 | 4 | 0.4973 | 0.4198 | 0.24 | 0.42 | 0.51 | 3911 |
| 1 | 1.0 | IVF-SQ8 | 8 | 0.4973 | — | 0.35 | 0.61 | 0.80 | 2624 |
| 2 | 2.0 | IVF-Flat | — | 0.6482 | 0.5695 | 1.34 | 2.58 | 2.95 | 684 |
| 2 | 2.0 | IVF-SQ8 | 2 | 0.6482 | — | 0.51 | 1.20 | 1.79 | 1686 |
| 2 | 2.0 | IVF-SQ8 | 4 | 0.6482 | 0.5695 | 0.38 | 0.64 | 0.78 | 2450 |
| 2 | 2.0 | IVF-SQ8 | 8 | 0.6482 | — | 0.53 | 1.01 | 1.20 | 1742 |
| 4 | 4.0 | IVF-Flat | — | 0.7797 | 0.7108 | 2.66 | 4.56 | 5.55 | 354 |
| 4 | 4.0 | IVF-SQ8 | 2 | 0.7797 | — | 0.85 | 2.13 | 3.86 | 978 |
| 4 | 4.0 | IVF-SQ8 | 4 | 0.7797 | 0.7108 | 0.61 | 0.98 | 1.14 | 1568 |
| 4 | 4.0 | IVF-SQ8 | 8 | 0.7797 | — | 0.79 | 1.56 | 2.14 | 1143 |
| 8 | 8.0 | IVF-Flat | — | 0.8767 | 0.8217 | 5.12 | 7.81 | 8.51 | 189 |
| 8 | 8.0 | IVF-SQ8 | 2 | 0.8767 | — | 1.33 | 2.36 | 3.50 | 694 |
| 8 | 8.0 | IVF-SQ8 | 4 | 0.8767 | 0.8217 | 1.28 | 1.94 | 2.18 | 759 |
| 8 | 8.0 | IVF-SQ8 | 8 | 0.8767 | — | 1.31 | 2.26 | 2.81 | 713 |
| 16 | 16.0 | IVF-Flat | — | 0.9379 | 0.9022 | 10.16 | 14.08 | 15.26 | 97 |
| 16 | 16.0 | IVF-SQ8 | 2 | 0.9379 | — | 2.22 | 3.18 | 3.71 | 437 |
| 16 | 16.0 | IVF-SQ8 | 4 | 0.9379 | 0.9022 | 2.18 | 3.12 | 3.46 | 445 |
| 16 | 16.0 | IVF-SQ8 | 8 | 0.9379 | — | 2.27 | 3.19 | 3.95 | 428 |
| 32 | 32.0 | IVF-Flat | — | 0.9727 | 0.9570 | 20.36 | 24.94 | 27.41 | 49 |
| 32 | 32.0 | IVF-SQ8 | 2 | 0.9727 | — | 4.22 | 5.33 | 6.07 | 235 |
| 32 | 32.0 | IVF-SQ8 | 4 | 0.9727 | 0.9570 | 4.16 | 5.14 | 5.57 | 238 |
| 32 | 32.0 | IVF-SQ8 | 8 | 0.9727 | — | 4.38 | 5.47 | 6.39 | 227 |
| 64 | 64.0 | IVF-Flat | — | 0.9935 | 0.9907 | 37.60 | 43.01 | 45.62 | 26 |
| 64 | 64.0 | IVF-SQ8 | 2 | 0.9935 | — | 8.99 | 11.86 | 13.13 | 108 |
| 64 | 64.0 | IVF-SQ8 | 4 | 0.9935 | 0.9907 | 8.50 | 9.81 | 10.57 | 117 |
| 64 | 64.0 | IVF-SQ8 | 8 | 0.9935 | — | 8.63 | 12.01 | 13.64 | 110 |

Resumen de los resultados (FNV-1a de ids y distancias): `7b883e220b034396`.

### cohere, 100000 filas de 768 dimensiones, dot

Paso 3, HNSW externo (`hnsw_rs`): M 16 (2·M en la capa 0), ef_construction 200, construido en 184.3 s en todos los núcleos; con identificadores de 4 bytes, los enlaces de la capa 0 ocuparían 12.2 MiB (2·M por vector) y los vectores 293.0 MiB, una estimación compacta: `hnsw_rs` guarda punteros compartidos y su propia copia de los vectores; el grafo: el RSS creció 566.0 MiB al construirlo (de 596.9 a 1162.9 MiB, pico del proceso 5865.7 MiB; incluye los temporales que el asignador no devolvió). Sin resumen: la construcción en paralelo no es determinista.

| ef | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|
| 10 | 0.7662 | 0.9010 | 3.24 | 5.10 | 5.88 | 293 |
| 20 | 0.8654 | 0.9010 | 1.52 | 2.86 | 3.46 | 602 |
| 40 | 0.9336 | 0.9010 | 1.41 | 2.16 | 2.55 | 677 |
| 80 | 0.9680 | 0.9010 | 1.92 | 2.63 | 3.21 | 516 |
| 160 | 0.9874 | 0.9420 | 3.01 | 4.02 | 4.53 | 330 |
| 320 | 0.9961 | 0.9763 | 5.10 | 7.05 | 7.78 | 195 |
| 640 | 0.9988 | 0.9915 | 8.50 | 11.83 | 12.89 | 115 |

