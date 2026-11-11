# Banco comparativo de Krummi

- Fecha: 2026-11-11 09:33 (UTC+00:00)
- Ejecución: completa
- Sistema: Ubuntu 24.04 (Linux 5.15.133.1-microsoft-standard-WSL2)
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Memoria: 31 GiB
- Disco: Virtual Disk, ext4
- Krummi: desconocido
- Motores: Voron 7.2.7 (.NET 10.0.14), LMDB 0.9.70 (heed 0.23.1), redb 4.4.0, canopydb 0.2.5, fjall 3.1.13
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 28 de 29 umbrales. Incumple: `tabla-intervalos`.

## Operaciones por segundo

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall | Krummi / Voron | Umbral |
|---|---|---|---|---|---|---|---|---|
| carga-en-orden | 148.409 | 146.907 | 115.337 | 96.482 | 150.419 | 137.264 | 1,01 | cumple |
| carga-al-azar | 65.549 | 59.875 | 31.943 | 24.023 | 84.216 | 106.529 | 1,09 | cumple |
| carga-masiva | 379.085 | 323.583 | 508.904 | 317.401 | 307.920 | 249.188 | 1,17 | cumple |
| escrituras-sueltas | 188 | 182 | 203 | 364 | 361 | 194 | 1,03 | cumple |
| escrituras-concurrentes-4 | 391 | 180 | 200 | 377 | 372 | 192 | 2,16 | cumple |
| escrituras-concurrentes-16 | 1.539 | 191 | 202 | 368 | 364 | 193 | 8,06 | cumple |
| lecturas-al-azar | 772.276 | 766.227 | 894.689 | 764.022 | 905.652 | 238.105 | 1,00 | cumple |
| lecturas-al-azar-4 | 3.376.617 | 3.036.650 | 3.655.423 | 2.343.687 | 2.703.062 | 814.066 | 1,11 | cumple |
| lecturas-al-azar-16 | 3.344.524 | 3.352.583 | 2.950.844 | 2.364.184 | 1.849.000 | 714.764 | 0,99 | cumple |
| recorrido | 16.754.081 | 11.982.863 | 14.038.885 | 8.681.133 | 6.866.299 | 2.573.531 | 1,39 | cumple |
| intervalos | 13.503.886 | 14.581.357 | 12.971.953 | 7.045.878 | 5.755.839 | 1.815.725 | 0,92 | cumple |
| lecturas-con-escritor | 2.973.385 | 2.547.862 | 2.133.041 | 1.957.122 | 2.349.821 | 773.223 | 1,16 | cumple |
| escritor-con-lectores | 14.860 | 13.637 | 7.199 | 11.055 | 13.747 | 16.391 | 1,08 | cumple |
| borrado | 70.482 | 57.509 | 24.119 | 27.017 | 77.328 | 117.965 | 1,22 | cumple |
| reutilizacion | 67.248 | 57.588 | 25.909 | 23.393 | 74.961 | 114.236 | 1,16 | cumple |
| apertura | 403 | 3 | 5.965 | 86 | 9 | 1 | 149,94 | cumple |
| valores-grandes-carga | 3.445 | 3.247 | 4.297 | 2.940 | 4.221 | 5.387 | 1,06 | cumple |
| valores-grandes-lectura | 140.746 | 88.931 | 165.178 | 147.070 | 127.625 | 124.524 | 1,58 | cumple |
| multivalor-carga | 64.651 | 61.519 | 32.097 | 37.072 | no aplica | no aplica | 1,05 | cumple |
| multivalor-lectura | 8.783.650 | 6.812.927 | 8.793.572 | 6.055.229 | no aplica | no aplica | 1,28 | cumple |
| tamano-fijo-carga | 63.087 | 64.233 | 35.450 | 37.050 | no aplica | no aplica | 0,98 | cumple |
| tamano-fijo-lectura | 1.934.541 | 1.553.379 | 1.487.816 | 1.223.342 | no aplica | no aplica | 1,24 | cumple |
| tabla-insercion-en-orden | 125.871 | 114.659 | no aplica | no aplica | no aplica | no aplica | 1,09 | cumple |
| tabla-insercion-al-azar | 58.507 | 57.871 | no aplica | no aplica | no aplica | no aplica | 1,01 | cumple |
| tabla-carga-masiva | 407.083 | 381.196 | no aplica | no aplica | no aplica | no aplica | 1,06 | cumple |
| tabla-lecturas | 571.643 | 645.938 | no aplica | no aplica | no aplica | no aplica | 0,88 | cumple |
| tabla-intervalos | 1.988.999 | 10.099.711 | no aplica | no aplica | no aplica | no aplica | 0,19 | incumple |
| tabla-actualizacion | 60.843 | 69.397 | no aplica | no aplica | no aplica | no aplica | 0,87 | cumple |
| tabla-borrado | 72.272 | 37.645 | no aplica | no aplica | no aplica | no aplica | 1,91 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall |
|---|---|---|---|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 | 133.054.464 | 134.746.112 | 139.309.355 | 160.711.632 |
| carga-al-azar | 268.435.456 | 671.088.964 | 200.876.032 | 269.488.128 | 363.462.955 | 160.945.569 |
| carga-masiva | 268.435.456 | 402.719.044 | 189.239.296 | 269.488.128 | 344.170.216 | 191.723.778 |
| borrado | 268.435.456 | 671.088.964 | 201.248.768 | 585.728 | 378.249.515 | 203.692.607 |
| reutilizacion | 268.435.456 | 671.088.964 | 208.777.216 | 268.701.696 | 383.684.907 | 238.204.041 |
| valores-grandes-carga | 536.870.912 | 939.524.420 | 285.802.496 | 538.972.160 | 625.836.331 | 336.042.541 |
| multivalor-carga | 33.554.432 | 335.544.644 | 54.071.296 | 67.375.104 | no aplica | no aplica |
| tamano-fijo-carga | 33.554.432 | 570.425.668 | 46.190.592 | 44.044.288 | no aplica | no aplica |
| tabla-insercion-en-orden | 134.217.728 | 369.099.076 | no aplica | no aplica | no aplica | no aplica |
| tabla-insercion-al-azar | 268.435.456 | 671.088.964 | no aplica | no aplica | no aplica | no aplica |
| tabla-carga-masiva | 134.217.728 | 402.719.044 | no aplica | no aplica | no aplica | no aplica |
| tabla-borrado | 134.217.728 | 671.088.964 | no aplica | no aplica | no aplica | no aplica |

## Opciones cambiadas

- LMDB: tamaño del mapa de 1 GiB; claves duplicadas ordenadas en W19 y W20; claves enteras en W21 y W22.
- canopydb: confirmación que espera al disco del diario.
- fjall: caché de bloques de 1 GiB; llevar el diario al disco (SyncData) en cada confirmación.
