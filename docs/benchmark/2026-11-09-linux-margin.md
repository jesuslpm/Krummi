# Banco comparativo de Krummi

- Fecha: 2026-11-09 11:00 (UTC+00:00)
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

Krummi cumple 22 de 22 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall | Krummi / Voron | Umbral |
|---|---|---|---|---|---|---|---|---|
| carga-en-orden | 151.929 | 138.071 | 116.271 | 131.905 | 151.970 | 129.549 | 1,10 | cumple |
| carga-al-azar | 67.721 | 61.378 | 31.233 | 27.915 | 87.724 | 100.653 | 1,10 | cumple |
| carga-masiva | 379.414 | 312.161 | 523.032 | 328.984 | 308.914 | 246.183 | 1,21 | cumple |
| escrituras-sueltas | 196 | 188 | 200 | 378 | 384 | 198 | 1,04 | cumple |
| escrituras-concurrentes-4 | 404 | 189 | 207 | 376 | 381 | 195 | 2,13 | cumple |
| escrituras-concurrentes-16 | 1.581 | 191 | 206 | 370 | 384 | 192 | 8,27 | cumple |
| lecturas-al-azar | 829.537 | 781.838 | 869.228 | 739.789 | 936.729 | 239.898 | 1,06 | cumple |
| lecturas-al-azar-4 | 3.504.865 | 3.207.341 | 3.785.799 | 2.300.833 | 2.741.376 | 826.705 | 1,09 | cumple |
| lecturas-al-azar-16 | 3.560.451 | 3.607.274 | 4.873.779 | 2.287.185 | 1.955.378 | 824.130 | 0,98 | cumple |
| recorrido | 17.880.567 | 9.409.680 | 11.790.112 | 12.402.375 | 6.940.169 | 2.588.808 | 1,90 | cumple |
| intervalos | 13.579.471 | 14.939.228 | 12.563.622 | 9.541.645 | 6.081.041 | 1.848.726 | 0,90 | cumple |
| lecturas-con-escritor | 2.806.337 | 2.540.355 | 3.054.865 | 1.944.862 | 2.376.158 | 769.738 | 1,10 | cumple |
| escritor-con-lectores | 14.953 | 13.324 | 8.767 | 11.489 | 14.200 | 16.464 | 1,12 | cumple |
| borrado | 70.543 | 57.833 | 28.450 | 24.504 | 76.746 | 119.470 | 1,21 | cumple |
| reutilizacion | 66.664 | 57.860 | 23.703 | 23.904 | 72.591 | 115.122 | 1,15 | cumple |
| apertura | 242 | 3 | 6.626 | 53 | 11 | 1 | 89,99 | cumple |
| valores-grandes-carga | 4.094 | 2.747 | 4.844 | 2.407 | 4.978 | 5.018 | 1,49 | cumple |
| valores-grandes-lectura | 138.221 | 88.059 | 140.825 | 134.626 | 154.634 | 126.553 | 1,56 | cumple |
| multivalor-carga | 64.078 | 63.130 | 36.361 | 35.010 | no aplica | no aplica | 1,01 | cumple |
| multivalor-lectura | 8.663.188 | 6.751.316 | 8.961.846 | 6.164.196 | no aplica | no aplica | 1,28 | cumple |
| tamano-fijo-carga | 60.553 | 62.840 | 34.861 | 36.830 | no aplica | no aplica | 0,96 | cumple |
| tamano-fijo-lectura | 2.002.283 | 1.584.540 | 1.483.942 | 1.414.266 | no aplica | no aplica | 1,26 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall |
|---|---|---|---|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 | 133.054.464 | 134.746.112 | 139.309.355 | 160.711.632 |
| carga-al-azar | 268.435.456 | 671.088.964 | 200.876.032 | 269.488.128 | 358.301.995 | 160.945.569 |
| carga-masiva | 268.435.456 | 402.719.044 | 189.239.296 | 269.488.128 | 344.170.216 | 191.723.778 |
| borrado | 268.435.456 | 671.088.964 | 201.248.768 | 585.728 | 371.011.883 | 203.692.607 |
| reutilizacion | 268.435.456 | 671.088.964 | 208.777.216 | 268.701.696 | 404.566.315 | 238.204.041 |
| valores-grandes-carga | 536.870.912 | 939.524.420 | 285.802.496 | 538.972.160 | 625.836.331 | 269.110.709 |
| multivalor-carga | 33.554.432 | 335.544.644 | 54.071.296 | 67.375.104 | no aplica | no aplica |
| tamano-fijo-carga | 33.554.432 | 570.425.668 | 46.190.592 | 44.044.288 | no aplica | no aplica |

## Opciones cambiadas

- LMDB: tamaño del mapa de 1 GiB; claves duplicadas ordenadas en W19 y W20; claves enteras en W21 y W22.
- canopydb: confirmación que espera al disco del diario.
- fjall: caché de bloques de 1 GiB; llevar el diario al disco (SyncData) en cada confirmación.
