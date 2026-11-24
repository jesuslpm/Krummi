# Experimento ANN: Linux (WSL), memoria a 100 000 filas (código de 96ee454)

Salida de `krummi-ann-lab`, sin editar. Informe: [../2026-11-24-ann-experiment.md](../2026-11-24-ann-experiment.md).

### sift, 100000 filas de 128 dimensiones, l2

Búsqueda exacta (100 consultas): p50 7.67 ms, p95 8.34 ms, p99 9.06 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 12.2 MiB en 0.1 s; 265 códigos con algún componente saturado (371 componentes); el RSS creció 25.4 MiB al construirlo (de 54.7 a 80.1 MiB, pico del proceso 982.9 MiB; incluye los temporales que el asignador no devolvió).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9913 | 0.9940 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 2.38 ms, p95 2.73 ms, p99 3.15 ms.

Paso 2, IVF: 100 listas, muestra de 25600 vectores, construido en 0.7 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 2.12 veces la media; IVF-SQ8 12.6 MiB, IVF-Flat 49.3 MiB; 265 códigos con algún componente saturado; los dos índices juntos: el RSS creció 138.3 MiB al construirlo (de 89.4 a 227.7 MiB, pico del proceso 982.9 MiB; incluye los temporales que el asignador no devolvió).

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.0 | IVF-Flat | — | 0.5132 | 0.4295 | 0.11 | 0.20 | 0.25 | 8180 |
| 1 | 1.0 | IVF-SQ8 | 2 | 0.5132 | — | 0.05 | 0.08 | 0.11 | 18667 |
| 1 | 1.0 | IVF-SQ8 | 4 | 0.5132 | 0.4295 | 0.05 | 0.07 | 0.08 | 18995 |
| 1 | 1.0 | IVF-SQ8 | 8 | 0.5132 | — | 0.07 | 0.10 | 0.11 | 13169 |
| 2 | 2.0 | IVF-Flat | — | 0.7142 | 0.6212 | 0.18 | 0.28 | 0.35 | 5249 |
| 2 | 2.0 | IVF-SQ8 | 2 | 0.7142 | — | 0.08 | 0.11 | 0.13 | 12449 |
| 2 | 2.0 | IVF-SQ8 | 4 | 0.7142 | 0.6212 | 0.07 | 0.11 | 0.13 | 13004 |
| 2 | 2.0 | IVF-SQ8 | 8 | 0.7142 | — | 0.11 | 0.16 | 0.20 | 8453 |
| 4 | 4.0 | IVF-Flat | — | 0.8700 | 0.7966 | 0.38 | 0.59 | 0.75 | 2498 |
| 4 | 4.0 | IVF-SQ8 | 2 | 0.8700 | — | 0.17 | 0.26 | 0.33 | 5746 |
| 4 | 4.0 | IVF-SQ8 | 4 | 0.8700 | 0.7966 | 0.13 | 0.19 | 0.23 | 7257 |
| 4 | 4.0 | IVF-SQ8 | 8 | 0.8700 | — | 0.17 | 0.24 | 0.28 | 5655 |
| 8 | 8.0 | IVF-Flat | — | 0.9641 | 0.9273 | 0.67 | 0.90 | 0.97 | 1467 |
| 8 | 8.0 | IVF-SQ8 | 2 | 0.9641 | — | 0.30 | 0.47 | 0.54 | 3159 |
| 8 | 8.0 | IVF-SQ8 | 4 | 0.9641 | 0.9273 | 0.22 | 0.31 | 0.34 | 4352 |
| 8 | 8.0 | IVF-SQ8 | 8 | 0.9641 | — | 0.30 | 0.46 | 0.52 | 3250 |
| 16 | 16.0 | IVF-Flat | — | 0.9952 | 0.9884 | 1.31 | 1.71 | 2.01 | 758 |
| 16 | 16.0 | IVF-SQ8 | 2 | 0.9952 | — | 0.46 | 0.78 | 0.99 | 2062 |
| 16 | 16.0 | IVF-SQ8 | 4 | 0.9952 | 0.9884 | 0.39 | 0.55 | 0.69 | 2453 |
| 16 | 16.0 | IVF-SQ8 | 8 | 0.9952 | — | 0.45 | 0.69 | 0.85 | 2131 |
| 32 | 32.0 | IVF-Flat | — | 1.0000 | 0.9996 | 2.52 | 3.18 | 3.67 | 402 |
| 32 | 32.0 | IVF-SQ8 | 2 | 1.0000 | — | 0.80 | 1.15 | 1.47 | 1239 |
| 32 | 32.0 | IVF-SQ8 | 4 | 1.0000 | 0.9996 | 0.76 | 0.99 | 1.26 | 1304 |
| 32 | 32.0 | IVF-SQ8 | 8 | 1.0000 | — | 0.81 | 1.27 | 1.53 | 1191 |
| 64 | 64.0 | IVF-Flat | — | 1.0000 | 1.0000 | 4.76 | 5.72 | 6.22 | 210 |
| 64 | 64.0 | IVF-SQ8 | 2 | 1.0000 | — | 1.36 | 1.77 | 1.99 | 724 |
| 64 | 64.0 | IVF-SQ8 | 4 | 1.0000 | 1.0000 | 1.39 | 1.67 | 2.05 | 705 |
| 64 | 64.0 | IVF-SQ8 | 8 | 1.0000 | — | 1.41 | 1.96 | 2.26 | 691 |

Resumen de los resultados (FNV-1a de ids y distancias): `d5f33b1ebc879582`.

### glove, 100000 filas de 100 dimensiones, coseno

Búsqueda exacta (100 consultas): p50 5.76 ms, p95 6.81 ms, p99 7.23 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 9.5 MiB en 0.0 s; 482 códigos con algún componente saturado (482 componentes); el RSS creció 20.0 MiB al construirlo (de 43.9 a 63.8 MiB, pico del proceso 916.8 MiB; incluye los temporales que el asignador no devolvió).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9842 | 0.9880 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 1.53 ms, p95 1.82 ms, p99 2.44 ms.

Paso 2, IVF: 100 listas, muestra de 25600 vectores, construido en 0.6 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 2.24 veces la media; IVF-SQ8 10.0 MiB, IVF-Flat 38.6 MiB; 482 códigos con algún componente saturado; los dos índices juntos: el RSS creció 105.8 MiB al construirlo (de 73.1 a 178.9 MiB, pico del proceso 916.8 MiB; incluye los temporales que el asignador no devolvió).

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.0 | IVF-Flat | — | 0.4242 | 0.3524 | 0.09 | 0.19 | 0.21 | 9363 |
| 1 | 1.0 | IVF-SQ8 | 2 | 0.4242 | — | 0.04 | 0.07 | 0.09 | 24097 |
| 1 | 1.0 | IVF-SQ8 | 4 | 0.4242 | 0.3524 | 0.04 | 0.07 | 0.08 | 22615 |
| 1 | 1.0 | IVF-SQ8 | 8 | 0.4242 | — | 0.06 | 0.09 | 0.13 | 15915 |
| 2 | 2.0 | IVF-Flat | — | 0.5695 | 0.4871 | 0.15 | 0.25 | 0.28 | 6411 |
| 2 | 2.0 | IVF-SQ8 | 2 | 0.5695 | — | 0.06 | 0.10 | 0.13 | 15001 |
| 2 | 2.0 | IVF-SQ8 | 4 | 0.5695 | 0.4871 | 0.07 | 0.10 | 0.12 | 15035 |
| 2 | 2.0 | IVF-SQ8 | 8 | 0.5695 | — | 0.08 | 0.12 | 0.14 | 11662 |
| 4 | 4.0 | IVF-Flat | — | 0.7083 | 0.6204 | 0.28 | 0.41 | 0.48 | 3506 |
| 4 | 4.0 | IVF-SQ8 | 2 | 0.7083 | — | 0.11 | 0.16 | 0.19 | 8674 |
| 4 | 4.0 | IVF-SQ8 | 4 | 0.7083 | 0.6204 | 0.11 | 0.15 | 0.17 | 9336 |
| 4 | 4.0 | IVF-SQ8 | 8 | 0.7083 | — | 0.14 | 0.20 | 0.23 | 7026 |
| 8 | 8.0 | IVF-Flat | — | 0.8277 | 0.7495 | 0.57 | 0.76 | 0.93 | 1758 |
| 8 | 8.0 | IVF-SQ8 | 2 | 0.8277 | — | 0.21 | 0.32 | 0.36 | 4509 |
| 8 | 8.0 | IVF-SQ8 | 4 | 0.8277 | 0.7495 | 0.17 | 0.23 | 0.29 | 5688 |
| 8 | 8.0 | IVF-SQ8 | 8 | 0.8277 | — | 0.22 | 0.33 | 0.43 | 4283 |
| 16 | 16.0 | IVF-Flat | — | 0.9148 | 0.8639 | 1.06 | 1.35 | 1.60 | 924 |
| 16 | 16.0 | IVF-SQ8 | 2 | 0.9148 | — | 0.40 | 0.53 | 0.60 | 2459 |
| 16 | 16.0 | IVF-SQ8 | 4 | 0.9148 | 0.8639 | 0.44 | 0.57 | 0.67 | 2262 |
| 16 | 16.0 | IVF-SQ8 | 8 | 0.9148 | — | 0.52 | 0.93 | 1.06 | 1719 |
| 32 | 32.0 | IVF-Flat | — | 0.9719 | 0.9474 | 2.32 | 2.89 | 3.23 | 422 |
| 32 | 32.0 | IVF-SQ8 | 2 | 0.9719 | — | 0.70 | 1.09 | 1.29 | 1336 |
| 32 | 32.0 | IVF-SQ8 | 4 | 0.9719 | 0.9474 | 0.58 | 0.71 | 0.79 | 1689 |
| 32 | 32.0 | IVF-SQ8 | 8 | 0.9719 | — | 0.68 | 1.03 | 1.26 | 1382 |
| 64 | 64.0 | IVF-Flat | — | 0.9976 | 0.9935 | 4.10 | 4.84 | 5.48 | 239 |
| 64 | 64.0 | IVF-SQ8 | 2 | 0.9976 | — | 1.18 | 2.03 | 2.52 | 785 |
| 64 | 64.0 | IVF-SQ8 | 4 | 0.9976 | 0.9935 | 1.11 | 1.35 | 1.59 | 886 |
| 64 | 64.0 | IVF-SQ8 | 8 | 0.9976 | — | 1.12 | 1.73 | 1.89 | 832 |

Resumen de los resultados (FNV-1a de ids y distancias): `e01e565fb310de5a`.

### cohere, 100000 filas de 768 dimensiones, l2

Búsqueda exacta (100 consultas): p50 46.01 ms, p95 51.41 ms, p99 53.71 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 73.2 MiB en 0.4 s; 3516 códigos con algún componente saturado (4057 componentes); el RSS creció 149.2 MiB al construirlo (de 301.3 a 450.4 MiB, pico del proceso 5865.5 MiB; incluye los temporales que el asignador no devolvió).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9942 | 0.9941 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 14.08 ms, p95 16.46 ms, p99 17.21 ms.

Paso 2, IVF: 100 listas, muestra de 25600 vectores, construido en 4.4 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 3.54 veces la media; IVF-SQ8 73.9 MiB, IVF-Flat 293.6 MiB; 3516 códigos con algún componente saturado; los dos índices juntos: el RSS creció 792.2 MiB al construirlo (de 459.3 a 1251.5 MiB, pico del proceso 5865.5 MiB; incluye los temporales que el asignador no devolvió).

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.0 | IVF-Flat | — | 0.4810 | 0.3620 | 0.68 | 1.31 | 1.83 | 1321 |
| 1 | 1.0 | IVF-SQ8 | 2 | 0.4810 | — | 0.38 | 0.73 | 0.95 | 2427 |
| 1 | 1.0 | IVF-SQ8 | 4 | 0.4810 | 0.3620 | 0.28 | 0.49 | 0.65 | 3271 |
| 1 | 1.0 | IVF-SQ8 | 8 | 0.4810 | — | 0.40 | 0.71 | 0.94 | 2332 |
| 2 | 2.0 | IVF-Flat | — | 0.6520 | 0.5181 | 1.25 | 2.39 | 2.89 | 732 |
| 2 | 2.0 | IVF-SQ8 | 2 | 0.6520 | — | 0.53 | 1.14 | 1.54 | 1665 |
| 2 | 2.0 | IVF-SQ8 | 4 | 0.6520 | 0.5181 | 0.49 | 0.85 | 1.00 | 1893 |
| 2 | 2.0 | IVF-SQ8 | 8 | 0.6520 | — | 0.59 | 1.14 | 1.48 | 1563 |
| 4 | 4.0 | IVF-Flat | — | 0.7943 | 0.6702 | 2.57 | 4.36 | 4.84 | 369 |
| 4 | 4.0 | IVF-SQ8 | 2 | 0.7943 | — | 1.03 | 2.69 | 4.84 | 806 |
| 4 | 4.0 | IVF-SQ8 | 4 | 0.7943 | 0.6702 | 0.81 | 1.32 | 1.45 | 1176 |
| 4 | 4.0 | IVF-SQ8 | 8 | 0.7943 | — | 0.92 | 1.78 | 2.72 | 983 |
| 8 | 8.0 | IVF-Flat | — | 0.9001 | 0.8113 | 4.91 | 7.64 | 8.74 | 196 |
| 8 | 8.0 | IVF-SQ8 | 2 | 0.9001 | — | 1.39 | 2.17 | 2.84 | 688 |
| 8 | 8.0 | IVF-SQ8 | 4 | 0.9001 | 0.8113 | 1.40 | 2.08 | 2.34 | 697 |
| 8 | 8.0 | IVF-SQ8 | 8 | 0.9001 | — | 1.56 | 2.37 | 2.86 | 619 |
| 16 | 16.0 | IVF-Flat | — | 0.9636 | 0.9157 | 9.30 | 12.69 | 13.82 | 106 |
| 16 | 16.0 | IVF-SQ8 | 2 | 0.9636 | — | 2.76 | 3.83 | 4.31 | 358 |
| 16 | 16.0 | IVF-SQ8 | 4 | 0.9636 | 0.9157 | 2.77 | 3.84 | 4.78 | 351 |
| 16 | 16.0 | IVF-SQ8 | 8 | 0.9636 | — | 2.72 | 3.73 | 4.15 | 360 |
| 32 | 32.0 | IVF-Flat | — | 0.9903 | 0.9757 | 18.25 | 22.50 | 24.51 | 55 |
| 32 | 32.0 | IVF-SQ8 | 2 | 0.9903 | — | 5.33 | 6.79 | 7.56 | 186 |
| 32 | 32.0 | IVF-SQ8 | 4 | 0.9903 | 0.9757 | 5.20 | 6.43 | 7.14 | 191 |
| 32 | 32.0 | IVF-SQ8 | 8 | 0.9903 | — | 5.47 | 6.70 | 7.13 | 182 |
| 64 | 64.0 | IVF-Flat | — | 0.9994 | 0.9984 | 35.00 | 40.12 | 43.66 | 28 |
| 64 | 64.0 | IVF-SQ8 | 2 | 0.9994 | — | 10.82 | 17.40 | 20.44 | 86 |
| 64 | 64.0 | IVF-SQ8 | 4 | 0.9994 | 0.9984 | 10.13 | 12.14 | 13.65 | 97 |
| 64 | 64.0 | IVF-SQ8 | 8 | 0.9994 | — | 10.62 | 12.63 | 13.61 | 93 |

Resumen de los resultados (FNV-1a de ids y distancias): `755188a223c73701`.

### cohere, 100000 filas de 768 dimensiones, dot

Búsqueda exacta (100 consultas): p50 44.06 ms, p95 48.70 ms, p99 50.60 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 73.2 MiB en 0.3 s; 3516 códigos con algún componente saturado (4057 componentes); el RSS creció 0.0 MiB al construirlo (de 735.2 a 735.2 MiB, pico del proceso 5865.5 MiB; incluye los temporales que el asignador no devolvió).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9911 | 0.9932 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 9.35 ms, p95 10.69 ms, p99 11.74 ms.

Paso 2, IVF: 100 listas, muestra de 25600 vectores, construido en 4.4 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 3.54 veces la media; IVF-SQ8 73.9 MiB, IVF-Flat 293.6 MiB; 3516 códigos con algún componente saturado; los dos índices juntos: el RSS creció 540.7 MiB al construirlo (de 709.8 a 1250.5 MiB, pico del proceso 5865.5 MiB; incluye los temporales que el asignador no devolvió).

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 1.0 | IVF-Flat | — | 0.4973 | 0.4198 | 0.62 | 1.14 | 1.69 | 1432 |
| 1 | 1.0 | IVF-SQ8 | 2 | 0.4973 | — | 0.24 | 0.43 | 0.56 | 3913 |
| 1 | 1.0 | IVF-SQ8 | 4 | 0.4973 | 0.4198 | 0.20 | 0.32 | 0.42 | 4642 |
| 1 | 1.0 | IVF-SQ8 | 8 | 0.4973 | — | 0.32 | 0.53 | 0.69 | 2941 |
| 2 | 2.0 | IVF-Flat | — | 0.6482 | 0.5695 | 1.23 | 2.36 | 2.69 | 746 |
| 2 | 2.0 | IVF-SQ8 | 2 | 0.6482 | — | 0.44 | 0.92 | 1.20 | 2052 |
| 2 | 2.0 | IVF-SQ8 | 4 | 0.6482 | 0.5695 | 0.32 | 0.53 | 0.60 | 2914 |
| 2 | 2.0 | IVF-SQ8 | 8 | 0.6482 | — | 0.45 | 0.83 | 1.06 | 2057 |
| 4 | 4.0 | IVF-Flat | — | 0.7797 | 0.7108 | 2.39 | 3.99 | 4.43 | 398 |
| 4 | 4.0 | IVF-SQ8 | 2 | 0.7797 | — | 0.64 | 1.30 | 1.74 | 1428 |
| 4 | 4.0 | IVF-SQ8 | 4 | 0.7797 | 0.7108 | 0.54 | 0.87 | 0.98 | 1747 |
| 4 | 4.0 | IVF-SQ8 | 8 | 0.7797 | — | 0.69 | 1.27 | 1.72 | 1324 |
| 8 | 8.0 | IVF-Flat | — | 0.8767 | 0.8217 | 4.74 | 7.15 | 7.86 | 206 |
| 8 | 8.0 | IVF-SQ8 | 2 | 0.8767 | — | 1.06 | 1.66 | 1.96 | 901 |
| 8 | 8.0 | IVF-SQ8 | 4 | 0.8767 | 0.8217 | 1.08 | 1.62 | 1.85 | 905 |
| 8 | 8.0 | IVF-SQ8 | 8 | 0.8767 | — | 1.14 | 1.83 | 2.43 | 827 |
| 16 | 16.0 | IVF-Flat | — | 0.9379 | 0.9022 | 10.06 | 13.90 | 15.95 | 97 |
| 16 | 16.0 | IVF-SQ8 | 2 | 0.9379 | — | 2.15 | 3.03 | 3.74 | 454 |
| 16 | 16.0 | IVF-SQ8 | 4 | 0.9379 | 0.9022 | 2.10 | 2.89 | 3.36 | 464 |
| 16 | 16.0 | IVF-SQ8 | 8 | 0.9379 | — | 2.13 | 3.04 | 3.47 | 456 |
| 32 | 32.0 | IVF-Flat | — | 0.9727 | 0.9570 | 17.97 | 21.79 | 23.37 | 55 |
| 32 | 32.0 | IVF-SQ8 | 2 | 0.9727 | — | 3.81 | 4.73 | 5.18 | 259 |
| 32 | 32.0 | IVF-SQ8 | 4 | 0.9727 | 0.9570 | 3.83 | 4.76 | 5.19 | 258 |
| 32 | 32.0 | IVF-SQ8 | 8 | 0.9727 | — | 4.25 | 5.48 | 5.95 | 232 |
| 64 | 64.0 | IVF-Flat | — | 0.9935 | 0.9907 | 35.17 | 39.85 | 43.20 | 28 |
| 64 | 64.0 | IVF-SQ8 | 2 | 0.9935 | — | 7.77 | 9.28 | 10.00 | 127 |
| 64 | 64.0 | IVF-SQ8 | 4 | 0.9935 | 0.9907 | 8.23 | 9.73 | 10.70 | 120 |
| 64 | 64.0 | IVF-SQ8 | 8 | 0.9935 | — | 7.69 | 9.72 | 11.11 | 126 |

Resumen de los resultados (FNV-1a de ids y distancias): `7b883e220b034396`.

### glove, 100000 filas de 100 dimensiones, coseno

Paso 3, HNSW externo (`hnsw_rs`): M 16 (2·M en la capa 0), ef_construction 200, construido en 22.2 s en todos los núcleos; con identificadores de 4 bytes, los enlaces de la capa 0 ocuparían 12.2 MiB (2·M por vector) y los vectores 38.1 MiB, una estimación compacta: `hnsw_rs` guarda punteros compartidos y su propia copia de los vectores; el grafo: el RSS creció 302.4 MiB al construirlo (de 84.6 a 387.0 MiB, pico del proceso 917.0 MiB; incluye los temporales que el asignador no devolvió). Sin resumen: la construcción en paralelo no es determinista.

| ef | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|
| 10 | 0.5003 | 0.7633 | 0.27 | 0.55 | 0.74 | 3288 |
| 20 | 0.6207 | 0.7633 | 0.31 | 0.56 | 0.71 | 3009 |
| 40 | 0.7489 | 0.7633 | 0.49 | 0.86 | 1.10 | 1943 |
| 80 | 0.8439 | 0.7633 | 0.75 | 1.22 | 1.45 | 1301 |
| 160 | 0.9164 | 0.8302 | 1.28 | 1.97 | 2.43 | 779 |
| 320 | 0.9615 | 0.9060 | 2.13 | 3.21 | 3.77 | 470 |
| 640 | 0.9841 | 0.9579 | 4.00 | 6.06 | 6.89 | 251 |

### cohere, 100000 filas de 768 dimensiones, l2

Paso 3, HNSW externo (`hnsw_rs`): M 16 (2·M en la capa 0), ef_construction 200, construido en 101.2 s en todos los núcleos; con identificadores de 4 bytes, los enlaces de la capa 0 ocuparían 12.2 MiB (2·M por vector) y los vectores 293.0 MiB, una estimación compacta: `hnsw_rs` guarda punteros compartidos y su propia copia de los vectores; el grafo: el RSS creció 460.3 MiB al construirlo (de 596.8 a 1057.1 MiB, pico del proceso 5865.5 MiB; incluye los temporales que el asignador no devolvió). Sin resumen: la construcción en paralelo no es determinista.

| ef | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|
| 10 | 0.7000 | 0.8903 | 0.98 | 1.70 | 2.26 | 951 |
| 20 | 0.8058 | 0.8903 | 0.91 | 1.38 | 1.72 | 1044 |
| 40 | 0.8761 | 0.8903 | 1.14 | 1.57 | 1.82 | 860 |
| 80 | 0.9217 | 0.8903 | 1.57 | 2.07 | 2.40 | 629 |
| 160 | 0.9457 | 0.9270 | 2.23 | 2.87 | 3.09 | 444 |
| 320 | 0.9580 | 0.9576 | 4.20 | 6.35 | 7.30 | 229 |
| 640 | 0.9659 | 0.9726 | 6.62 | 8.24 | 8.89 | 150 |

### cohere, 100000 filas de 768 dimensiones, dot

Paso 3, HNSW externo (`hnsw_rs`): M 16 (2·M en la capa 0), ef_construction 200, construido en 107.3 s en todos los núcleos; con identificadores de 4 bytes, los enlaces de la capa 0 ocuparían 12.2 MiB (2·M por vector) y los vectores 293.0 MiB, una estimación compacta: `hnsw_rs` guarda punteros compartidos y su propia copia de los vectores; el grafo: el RSS creció 107.3 MiB al construirlo (de 1079.4 a 1186.6 MiB, pico del proceso 5865.5 MiB; incluye los temporales que el asignador no devolvió). Sin resumen: la construcción en paralelo no es determinista.

| ef | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|
| 10 | 0.7687 | 0.9000 | 0.46 | 0.94 | 1.25 | 1913 |
| 20 | 0.8631 | 0.9000 | 0.81 | 1.37 | 1.68 | 1159 |
| 40 | 0.9350 | 0.9000 | 1.06 | 1.66 | 2.33 | 890 |
| 80 | 0.9707 | 0.9000 | 1.74 | 2.51 | 3.14 | 562 |
| 160 | 0.9872 | 0.9412 | 2.89 | 4.02 | 4.83 | 341 |
| 320 | 0.9955 | 0.9764 | 4.76 | 6.55 | 7.52 | 209 |
| 640 | 0.9980 | 0.9914 | 7.60 | 10.48 | 11.57 | 130 |

