# Banco comparativo de Krummi

- Fecha: 2026-11-09 09:37 (UTC+00:00)
- Ejecución: completa
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: 908b65e36b5d
- Motores: Voron 7.2.7 (.NET 10.0.5), LMDB 0.9.70 (heed 0.23.1), redb 4.4.0, canopydb 0.2.5, fjall 3.1.13
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 22 de 22 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall | Krummi / Voron | Umbral |
|---|---|---|---|---|---|---|---|---|
| carga-en-orden | 990.853 | 689.444 | 167.810 | 255.308 | 339.277 | 256.451 | 1,43 | cumple |
| carga-al-azar | 117.244 | 110.276 | 29.829 | 18.889 | 175.441 | 203.895 | 1,06 | cumple |
| carga-masiva | 420.621 | 356.946 | 97.654 | 333.725 | 188.152 | 281.590 | 1,17 | cumple |
| escrituras-sueltas | 8.531 | 7.013 | 3.915 | 392 | 427 | 444 | 1,21 | cumple |
| escrituras-concurrentes-4 | 18.632 | 7.004 | 3.742 | 392 | 430 | 460 | 2,66 | cumple |
| escrituras-concurrentes-16 | 23.817 | 7.262 | 3.752 | 393 | 437 | 453 | 3,27 | cumple |
| lecturas-al-azar | 1.045.630 | 877.213 | 1.075.427 | 861.269 | 923.734 | 264.005 | 1,19 | cumple |
| lecturas-al-azar-4 | 4.280.823 | 3.382.927 | 4.526.048 | 2.617.952 | 2.625.028 | 945.216 | 1,26 | cumple |
| lecturas-al-azar-16 | 4.550.498 | 3.674.701 | 6.423.272 | 2.687.637 | 2.069.768 | 842.218 | 1,23 | cumple |
| recorrido | 19.030.582 | 9.521.969 | 13.977.139 | 8.051.627 | 7.237.126 | 2.530.608 | 1,99 | cumple |
| intervalos | 14.207.452 | 16.352.638 | 13.272.395 | 6.999.299 | 6.110.724 | 1.883.662 | 0,86 | cumple |
| lecturas-con-escritor | 2.449.920 | 2.109.135 | 2.052.302 | 2.110.879 | 1.382.538 | 763.798 | 1,16 | cumple |
| escritor-con-lectores | 83.927 | 68.156 | 9.367 | 7.874 | 26.807 | 39.652 | 1,23 | cumple |
| borrado | 120.698 | 102.633 | 27.087 | 16.103 | 42.234 | 208.968 | 1,17 | cumple |
| reutilizacion | 111.835 | 105.466 | 29.859 | 14.856 | 45.413 | 199.961 | 1,06 | cumple |
| apertura | 65 | 2 | 2.065 | 91 | 77 | 1 | 42,84 | cumple |
| valores-grandes-carga | 5.208 | 5.716 | 3.484 | 3.921 | 3.876 | 7.227 | 0,91 | cumple |
| valores-grandes-lectura | 140.795 | 110.700 | 153.931 | 160.643 | 138.709 | 117.979 | 1,27 | cumple |
| multivalor-carga | 117.570 | 122.666 | 42.073 | 26.054 | no aplica | no aplica | 0,95 | cumple |
| multivalor-lectura | 8.408.774 | 7.440.122 | 9.923.401 | 6.415.533 | no aplica | no aplica | 1,13 | cumple |
| tamano-fijo-carga | 149.325 | 154.377 | 43.181 | 27.512 | no aplica | no aplica | 0,96 | cumple |
| tamano-fijo-lectura | 2.022.533 | 1.802.552 | 1.276.775 | 1.412.389 | no aplica | no aplica | 1,12 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall |
|---|---|---|---|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 | 133.054.400 | 134.746.112 | 208.160.787 | 160.711.632 |
| carga-al-azar | 268.435.456 | 671.088.964 | 200.875.968 | 269.488.128 | 612.892.971 | 160.945.569 |
| carga-masiva | 268.435.456 | 402.719.044 | 189.239.232 | 269.488.128 | 344.170.216 | 191.723.778 |
| borrado | 268.435.456 | 671.088.964 | 201.248.704 | 585.728 | 553.906.475 | 203.736.765 |
| reutilizacion | 268.435.456 | 671.088.964 | 208.777.152 | 268.701.696 | 591.185.232 | 238.204.041 |
| valores-grandes-carga | 536.870.912 | 671.088.964 | 285.802.432 | 538.972.160 | 625.836.331 | 336.042.541 |
| multivalor-carga | 33.554.432 | 536.871.236 | 54.071.232 | 67.375.104 | no aplica | no aplica |
| tamano-fijo-carga | 33.554.432 | 570.425.668 | 46.190.528 | 44.044.288 | no aplica | no aplica |

## Opciones cambiadas

- LMDB: tamaño del mapa de 1 GiB; claves duplicadas ordenadas en W19 y W20; claves enteras en W21 y W22.
- canopydb: confirmación que espera al disco del diario.
- fjall: caché de bloques de 1 GiB; llevar el diario al disco (SyncData) en cada confirmación.
