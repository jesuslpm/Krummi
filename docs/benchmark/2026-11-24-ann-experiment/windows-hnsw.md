# Experimento ANN: Windows, HNSW externo (T7, código de d07655c)

Salida de `krummi-ann-lab`, sin editar. Informe: [../2026-11-24-ann-experiment.md](../2026-11-24-ann-experiment.md).

### cohere, 1000000 filas de 768 dimensiones, l2

Paso 3, HNSW externo (`hnsw_rs`): M 16 (2·M en la capa 0), ef_construction 200, construido en 1169.0 s en todos los núcleos; los enlaces de la capa 0 ocupan 122.1 MiB (2·M identificadores de 4 bytes por vector), más los vectores, 2929.7 MiB. Sin resumen: la construcción en paralelo no es determinista.

| ef | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|
| 10 | 0.6848 | 0.8692 | 0.64 | 1.08 | 1.46 | 1453 |
| 20 | 0.7971 | 0.8692 | 0.95 | 1.46 | 1.91 | 1011 |
| 40 | 0.8668 | 0.8692 | 1.47 | 2.24 | 2.63 | 649 |
| 80 | 0.9090 | 0.8692 | 2.66 | 3.71 | 4.21 | 373 |
| 160 | 0.9367 | 0.9104 | 4.98 | 6.36 | 7.01 | 203 |
| 320 | 0.9536 | 0.9467 | 8.75 | 11.19 | 12.36 | 115 |
| 640 | 0.9632 | 0.9659 | 16.31 | 20.74 | 22.87 | 62 |

### cohere, 1000000 filas de 768 dimensiones, dot

Paso 3, HNSW externo (`hnsw_rs`): M 16 (2·M en la capa 0), ef_construction 200, construido en 1790.8 s en todos los núcleos; los enlaces de la capa 0 ocupan 122.1 MiB (2·M identificadores de 4 bytes por vector), más los vectores, 2929.7 MiB. Sin resumen: la construcción en paralelo no es determinista.

| ef | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|
| 10 | 0.7699 | 0.8868 | 0.72 | 1.25 | 1.58 | 1270 |
| 20 | 0.8669 | 0.8868 | 1.11 | 2.62 | 3.49 | 808 |
| 40 | 0.9281 | 0.8868 | 2.37 | 3.32 | 3.99 | 450 |
| 80 | 0.9608 | 0.8868 | 3.76 | 5.89 | 6.69 | 242 |
| 160 | 0.9811 | 0.9259 | 7.29 | 9.65 | 10.67 | 136 |
| 320 | 0.9912 | 0.9625 | 12.85 | 16.31 | 17.82 | 78 |
| 640 | 0.9950 | 0.9814 | 21.99 | 27.95 | 30.16 | 46 |

### glove, 1192514 filas de 100 dimensiones, coseno

Paso 3, HNSW externo (`hnsw_rs`): M 16 (2·M en la capa 0), ef_construction 200, construido en 1625.6 s en todos los núcleos; los enlaces de la capa 0 ocupan 145.6 MiB (2·M identificadores de 4 bytes por vector), más los vectores, 454.9 MiB. Sin resumen: la construcción en paralelo no es determinista.

| ef | recall@10 | recall@100 | p50 ms | p95 ms | p99 ms | consultas/s |
|---|---|---|---|---|---|---|
| 10 | 0.4602 | 0.6950 | 0.32 | 0.53 | 0.85 | 2865 |
| 20 | 0.5776 | 0.6950 | 0.49 | 1.46 | 1.75 | 1588 |
| 40 | 0.6799 | 0.6950 | 1.35 | 3.01 | 3.93 | 715 |
| 80 | 0.7721 | 0.6950 | 3.19 | 4.28 | 5.24 | 348 |
| 160 | 0.8387 | 0.7518 | 4.64 | 6.63 | 7.37 | 206 |
| 320 | 0.8826 | 0.8140 | 8.39 | 11.81 | 12.93 | 116 |
| 640 | 0.9175 | 0.8664 | 13.37 | 19.24 | 21.29 | 74 |

