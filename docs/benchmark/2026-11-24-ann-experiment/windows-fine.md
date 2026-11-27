# Experimento ANN: Windows, barrido fino alrededor de recall@10 0.95 (T7, código de d07655c)

Salida de `krummi-ann-lab`, sin editar. Informe: [../2026-11-24-ann-experiment.md](../2026-11-24-ann-experiment.md).

### cohere, 1000000 filas de 768 dimensiones, l2

Búsqueda exacta (100 consultas): p50 879.94 ms, p95 895.51 ms, p99 906.77 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 732.4 MiB en 6.0 s; 4127 códigos con algún componente saturado (4270 componentes).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9936 | 0.9935 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 233.55 ms, p95 246.26 ms, p99 252.90 ms.

Paso 2, IVF: 1000 listas, muestra de 256000 vectores, construido en 578.8 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 3.96 veces la media; IVF-SQ8 739.2 MiB, IVF-Flat 2936.4 MiB; 4127 códigos con algún componente saturado.

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 40 | 4.0 | IVF-Flat | — | 0.9578 | 0.9169 | 46.36 | 60.87 | 66.75 | 21 |
| 40 | 4.0 | IVF-SQ8 | 2 | 0.9578 | — | 13.34 | 17.20 | 19.11 | 74 |
| 40 | 4.0 | IVF-SQ8 | 4 | 0.9578 | 0.9169 | 13.17 | 17.21 | 18.68 | 74 |
| 40 | 4.0 | IVF-SQ8 | 8 | 0.9578 | — | 13.30 | 17.54 | 19.15 | 74 |
| 48 | 4.8 | IVF-Flat | — | 0.9658 | 0.9318 | 56.05 | 71.85 | 77.81 | 18 |
| 48 | 4.8 | IVF-SQ8 | 2 | 0.9658 | — | 15.63 | 20.30 | 21.77 | 63 |
| 48 | 4.8 | IVF-SQ8 | 4 | 0.9658 | 0.9318 | 15.88 | 20.63 | 22.11 | 62 |
| 48 | 4.8 | IVF-SQ8 | 8 | 0.9658 | — | 15.74 | 20.15 | 21.67 | 63 |
| 56 | 5.6 | IVF-Flat | — | 0.9727 | 0.9429 | 64.29 | 82.00 | 87.87 | 15 |
| 56 | 5.6 | IVF-SQ8 | 2 | 0.9727 | — | 18.21 | 23.10 | 24.92 | 54 |
| 56 | 5.6 | IVF-SQ8 | 4 | 0.9727 | 0.9429 | 18.02 | 22.83 | 24.92 | 55 |
| 56 | 5.6 | IVF-SQ8 | 8 | 0.9727 | — | 18.22 | 23.07 | 24.42 | 54 |

Resumen de los resultados (FNV-1a de ids y distancias): `6a50e9fb0a98662d`.

### cohere, 1000000 filas de 768 dimensiones, dot

Búsqueda exacta (100 consultas): p50 876.25 ms, p95 889.36 ms, p99 896.79 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 732.4 MiB en 6.0 s; 4127 códigos con algún componente saturado (4270 componentes).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9930 | 0.9927 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 163.64 ms, p95 180.40 ms, p99 189.35 ms.

Paso 2, IVF: 1000 listas, muestra de 256000 vectores, construido en 595.6 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 3.96 veces la media; IVF-SQ8 739.2 MiB, IVF-Flat 2936.4 MiB; 4127 códigos con algún componente saturado.

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 72 | 7.2 | IVF-Flat | — | 0.9541 | 0.9208 | 82.28 | 101.02 | 108.30 | 12 |
| 72 | 7.2 | IVF-SQ8 | 2 | 0.9541 | — | 16.44 | 20.90 | 22.61 | 60 |
| 72 | 7.2 | IVF-SQ8 | 4 | 0.9541 | 0.9208 | 16.33 | 20.54 | 22.65 | 60 |
| 72 | 7.2 | IVF-SQ8 | 8 | 0.9541 | — | 16.60 | 20.66 | 22.86 | 59 |
| 80 | 8.0 | IVF-Flat | — | 0.9594 | 0.9278 | 91.61 | 111.72 | 118.27 | 11 |
| 80 | 8.0 | IVF-SQ8 | 2 | 0.9594 | — | 18.04 | 22.42 | 24.11 | 55 |
| 80 | 8.0 | IVF-SQ8 | 4 | 0.9594 | 0.9278 | 18.09 | 22.51 | 24.72 | 54 |
| 80 | 8.0 | IVF-SQ8 | 8 | 0.9594 | — | 17.94 | 22.23 | 23.88 | 55 |
| 96 | 9.6 | IVF-Flat | — | 0.9654 | 0.9397 | 109.52 | 130.45 | 138.08 | 9 |
| 96 | 9.6 | IVF-SQ8 | 2 | 0.9654 | — | 21.50 | 26.47 | 28.68 | 46 |
| 96 | 9.6 | IVF-SQ8 | 4 | 0.9654 | 0.9397 | 21.31 | 25.66 | 27.81 | 46 |
| 96 | 9.6 | IVF-SQ8 | 8 | 0.9654 | — | 21.59 | 26.02 | 27.89 | 46 |
| 112 | 11.2 | IVF-Flat | — | 0.9712 | 0.9486 | 127.61 | 149.53 | 157.07 | 8 |
| 112 | 11.2 | IVF-SQ8 | 2 | 0.9712 | — | 24.64 | 29.05 | 32.04 | 40 |
| 112 | 11.2 | IVF-SQ8 | 4 | 0.9712 | 0.9486 | 24.70 | 29.40 | 30.74 | 40 |
| 112 | 11.2 | IVF-SQ8 | 8 | 0.9712 | — | 24.78 | 29.65 | 32.09 | 40 |

Resumen de los resultados (FNV-1a de ids y distancias): `6782aacfc99c6f3c`.

### glove, 1192514 filas de 100 dimensiones, coseno

Búsqueda exacta (100 consultas): p50 140.25 ms, p95 145.56 ms, p99 151.92 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 113.7 MiB en 1.1 s; 677 códigos con algún componente saturado (677 componentes).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9786 | 0.9838 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 36.99 ms, p95 41.86 ms, p99 43.13 ms.

Paso 2, IVF: 1092 listas, muestra de 279552 vectores, construido en 88.9 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 2.73 veces la media; IVF-SQ8 118.7 MiB, IVF-Flat 459.9 MiB; 677 códigos con algún componente saturado.

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 160 | 14.7 | IVF-Flat | — | 0.9572 | 0.9223 | 19.03 | 22.47 | 24.41 | 52 |
| 160 | 14.7 | IVF-SQ8 | 2 | 0.9572 | — | 5.02 | 6.18 | 6.95 | 195 |
| 160 | 14.7 | IVF-SQ8 | 4 | 0.9572 | 0.9223 | 5.05 | 6.14 | 6.82 | 195 |
| 160 | 14.7 | IVF-SQ8 | 8 | 0.9572 | — | 5.13 | 6.26 | 6.95 | 193 |
| 192 | 17.6 | IVF-Flat | — | 0.9666 | 0.9367 | 22.93 | 26.73 | 28.54 | 43 |
| 192 | 17.6 | IVF-SQ8 | 2 | 0.9666 | — | 6.03 | 7.33 | 7.94 | 163 |
| 192 | 17.6 | IVF-SQ8 | 4 | 0.9666 | 0.9367 | 5.95 | 7.24 | 7.83 | 166 |
| 192 | 17.6 | IVF-SQ8 | 8 | 0.9666 | — | 6.00 | 7.08 | 7.80 | 165 |
| 256 | 23.4 | IVF-Flat | — | 0.9792 | 0.9573 | 30.47 | 34.73 | 36.49 | 33 |
| 256 | 23.4 | IVF-SQ8 | 2 | 0.9792 | — | 8.02 | 9.69 | 10.27 | 123 |
| 256 | 23.4 | IVF-SQ8 | 4 | 0.9792 | 0.9573 | 7.96 | 9.49 | 10.18 | 124 |
| 256 | 23.4 | IVF-SQ8 | 8 | 0.9792 | — | 7.97 | 9.35 | 10.05 | 124 |

Resumen de los resultados (FNV-1a de ids y distancias): `e15c2bd65a08a6e8`.

