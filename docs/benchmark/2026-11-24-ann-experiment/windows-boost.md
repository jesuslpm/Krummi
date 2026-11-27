# Experimento ANN: Windows con turbo, conjuntos de 10 000 filas (T7, código de d07655c)

Salida de `krummi-ann-lab`, sin editar. Informe: [../2026-11-24-ann-experiment.md](../2026-11-24-ann-experiment.md).

### sift, 10000 filas de 128 dimensiones, l2

Búsqueda exacta (100 consultas): p50 0.81 ms, p95 0.86 ms, p99 0.99 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 1.2 MiB en 0.0 s; 223 códigos con algún componente saturado (367 componentes).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9936 | 0.9957 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 0.27 ms, p95 0.30 ms, p99 0.35 ms.

Paso 2, IVF: 10 listas, muestra de 2560 vectores, construido en 0.0 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 1.65 veces la media; IVF-SQ8 1.3 MiB, IVF-Flat 4.9 MiB; 223 códigos con algún componente saturado.

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 10.0 | IVF-Flat | — | 0.7487 | 0.6882 | 0.15 | 0.24 | 0.24 | 6742 |
| 1 | 10.0 | IVF-SQ8 | 2 | 0.7487 | — | 0.05 | 0.07 | 0.08 | 20315 |
| 1 | 10.0 | IVF-SQ8 | 4 | 0.7487 | 0.6882 | 0.05 | 0.07 | 0.07 | 19758 |
| 1 | 10.0 | IVF-SQ8 | 8 | 0.7487 | — | 0.06 | 0.09 | 0.09 | 15109 |
| 2 | 20.0 | IVF-Flat | — | 0.9302 | 0.8984 | 0.21 | 0.28 | 0.35 | 4796 |
| 2 | 20.0 | IVF-SQ8 | 2 | 0.9302 | — | 0.08 | 0.10 | 0.10 | 12667 |
| 2 | 20.0 | IVF-SQ8 | 4 | 0.9302 | 0.8984 | 0.08 | 0.10 | 0.12 | 12186 |
| 2 | 20.0 | IVF-SQ8 | 8 | 0.9302 | — | 0.10 | 0.12 | 0.13 | 10252 |
| 4 | 40.0 | IVF-Flat | — | 0.9987 | 0.9960 | 0.34 | 0.43 | 0.59 | 2886 |
| 4 | 40.0 | IVF-SQ8 | 2 | 0.9987 | — | 0.14 | 0.17 | 0.21 | 6781 |
| 4 | 40.0 | IVF-SQ8 | 4 | 0.9987 | 0.9960 | 0.13 | 0.15 | 0.20 | 7777 |
| 4 | 40.0 | IVF-SQ8 | 8 | 0.9987 | — | 0.15 | 0.18 | 0.21 | 6484 |
| 8 | 80.0 | IVF-Flat | — | 1.0000 | 1.0000 | 0.59 | 0.78 | 0.98 | 1629 |
| 8 | 80.0 | IVF-SQ8 | 2 | 1.0000 | — | 0.29 | 0.38 | 0.44 | 3368 |
| 8 | 80.0 | IVF-SQ8 | 4 | 1.0000 | 1.0000 | 0.20 | 0.25 | 0.32 | 4778 |
| 8 | 80.0 | IVF-SQ8 | 8 | 1.0000 | — | 0.25 | 0.32 | 0.34 | 3886 |

Resumen de los resultados (FNV-1a de ids y distancias): `9d63d3c2eba2f703`.

### cohere, 10000 filas de 768 dimensiones, l2

Búsqueda exacta (100 consultas): p50 4.62 ms, p95 5.19 ms, p99 5.40 ms.

Paso 1, escaneo SQ8 aproximado con reordenación: códigos de 7.3 MiB en 0.0 s; 2407 códigos con algún componente saturado (4388 componentes).

| r | recall@10 (10·r candidatos) | recall@100 (100·r candidatos) |
|---|---|---|
| 1 | 0.9922 | 0.9949 |
| 2 | 1.0000 | 1.0000 |
| 4 | 1.0000 | 1.0000 |
| 8 | 1.0000 | 1.0000 |
| 16 | 1.0000 | 1.0000 |

Latencia del escaneo con r = 4 y k = 10: p50 1.30 ms, p95 1.46 ms, p99 1.55 ms.

Paso 2, IVF: 10 listas, muestra de 2560 vectores, construido en 0.3 s (k-means de 10 iteraciones, cuantizador y asignación de todas las filas); lista mayor 1.47 veces la media; IVF-SQ8 7.4 MiB, IVF-Flat 29.4 MiB; 2407 códigos con algún componente saturado.

| nprobe | % listas | índice | r | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|---|---|---|
| 1 | 10.0 | IVF-Flat | — | 0.6186 | 0.4732 | 0.70 | 1.23 | 1.33 | 1358 |
| 1 | 10.0 | IVF-SQ8 | 2 | 0.6186 | — | 0.24 | 0.37 | 0.41 | 4037 |
| 1 | 10.0 | IVF-SQ8 | 4 | 0.6186 | 0.4732 | 0.20 | 0.26 | 0.28 | 5155 |
| 1 | 10.0 | IVF-SQ8 | 8 | 0.6186 | — | 0.28 | 0.37 | 0.40 | 3593 |
| 2 | 20.0 | IVF-Flat | — | 0.8164 | 0.6871 | 1.03 | 1.30 | 1.39 | 973 |
| 2 | 20.0 | IVF-SQ8 | 2 | 0.8164 | — | 0.46 | 0.66 | 0.69 | 2156 |
| 2 | 20.0 | IVF-SQ8 | 4 | 0.8164 | 0.6871 | 0.32 | 0.41 | 0.44 | 3088 |
| 2 | 20.0 | IVF-SQ8 | 8 | 0.8164 | — | 0.43 | 0.71 | 0.81 | 2152 |
| 4 | 40.0 | IVF-Flat | — | 0.9345 | 0.8664 | 1.95 | 2.30 | 2.53 | 514 |
| 4 | 40.0 | IVF-SQ8 | 2 | 0.9345 | — | 0.86 | 1.16 | 1.30 | 1181 |
| 4 | 40.0 | IVF-SQ8 | 4 | 0.9345 | 0.8664 | 0.56 | 0.67 | 0.77 | 1777 |
| 4 | 40.0 | IVF-SQ8 | 8 | 0.9345 | — | 0.71 | 1.14 | 1.26 | 1311 |
| 8 | 80.0 | IVF-Flat | — | 0.9951 | 0.9855 | 3.69 | 4.58 | 5.02 | 264 |
| 8 | 80.0 | IVF-SQ8 | 2 | 0.9951 | — | 1.10 | 1.99 | 2.19 | 777 |
| 8 | 80.0 | IVF-SQ8 | 4 | 0.9951 | 0.9855 | 1.02 | 1.20 | 1.39 | 962 |
| 8 | 80.0 | IVF-SQ8 | 8 | 0.9951 | — | 1.14 | 1.99 | 2.23 | 785 |

Resumen de los resultados (FNV-1a de ids y distancias): `ab99ede8b858e573`.

