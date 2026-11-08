# Banco comparativo de Krummi

- Fecha: 2026-11-08 15:31 (UTC+00:00)
- Ejecución: completa
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: 4a3a92eb2760 (con cambios sin confirmar)
- Motores: Voron 7.2.7 (.NET 10.0.5), LMDB 0.9.70 (heed 0.23.1), redb 4.4.0, canopydb 0.2.5, fjall 3.1.13
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 21 de 22 umbrales. Incumple: `valores-grandes-carga`.

## Operaciones por segundo

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall | Krummi / Voron | Umbral |
|---|---|---|---|---|---|---|---|---|
| carga-en-orden | 698.262 | 667.532 | 167.402 | 171.557 | 222.752 | 181.015 | 1,04 | cumple |
| carga-al-azar | 110.821 | 108.640 | 31.635 | 18.931 | 126.038 | 151.526 | 1,02 | cumple |
| carga-masiva | 278.666 | 336.811 | 97.596 | 321.558 | 188.667 | 275.831 | 0,82 | cumple |
| escrituras-sueltas | 8.299 | 7.053 | 3.723 | 253 | 269 | 273 | 1,17 | cumple |
| escrituras-concurrentes-4 | 19.100 | 6.774 | 3.674 | 253 | 264 | 277 | 2,81 | cumple |
| escrituras-concurrentes-16 | 22.937 | 6.713 | 3.665 | 250 | 269 | 277 | 3,41 | cumple |
| lecturas-al-azar | 1.000.424 | 855.314 | 1.014.554 | 870.866 | 931.509 | 260.769 | 1,16 | cumple |
| lecturas-al-azar-4 | 4.394.385 | 3.597.894 | 4.567.159 | 2.526.722 | 2.755.481 | 918.848 | 1,22 | cumple |
| lecturas-al-azar-16 | 6.824.629 | 5.227.513 | 7.290.376 | 2.666.125 | 2.166.817 | 951.695 | 1,30 | cumple |
| recorrido | 19.410.277 | 10.123.497 | 14.652.981 | 8.361.022 | 7.433.507 | 2.616.548 | 1,91 | cumple |
| intervalos | 14.670.941 | 16.826.083 | 13.455.758 | 6.926.890 | 5.997.383 | 1.936.033 | 0,87 | cumple |
| lecturas-con-escritor | 2.041.491 | 2.321.741 | 3.107.612 | 2.155.449 | 1.964.666 | 819.339 | 0,87 | cumple |
| escritor-con-lectores | 77.402 | 62.495 | 14.208 | 7.126 | 18.433 | 24.303 | 1,23 | cumple |
| borrado | 117.787 | 103.152 | 33.049 | 16.743 | 38.038 | 160.078 | 1,14 | cumple |
| reutilizacion | 111.128 | 104.350 | 22.284 | 15.275 | 40.481 | 152.437 | 1,06 | cumple |
| apertura | 63 | 2 | 2.025 | 56 | 104 | 1 | 41,27 | cumple |
| valores-grandes-carga | 3.173 | 5.017 | 1.893 | 3.329 | 3.454 | 5.720 | 0,63 | incumple |
| valores-grandes-lectura | 162.395 | 102.323 | 146.085 | 153.806 | 141.186 | 112.026 | 1,58 | cumple |
| multivalor-carga | 106.544 | 120.694 | 30.878 | 25.534 | no aplica | no aplica | 0,88 | cumple |
| multivalor-lectura | 8.483.376 | 5.627.453 | 9.504.547 | 6.424.241 | no aplica | no aplica | 1,50 | cumple |
| tamano-fijo-carga | 142.976 | 151.514 | 51.440 | 27.868 | no aplica | no aplica | 0,94 | cumple |
| tamano-fijo-lectura | 2.171.235 | 1.819.503 | 1.321.469 | 1.523.796 | no aplica | no aplica | 1,19 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall |
|---|---|---|---|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 | 133.054.400 | 134.746.112 | 208.160.787 | 160.711.632 |
| carga-al-azar | 268.435.456 | 671.088.964 | 200.875.968 | 269.488.128 | 563.167.531 | 160.945.569 |
| carga-masiva | 268.435.456 | 402.719.044 | 189.239.232 | 269.488.128 | 344.170.216 | 191.723.778 |
| borrado | 268.435.456 | 671.088.964 | 201.248.704 | 585.728 | 557.220.139 | 203.692.607 |
| reutilizacion | 268.435.456 | 671.088.964 | 208.777.152 | 268.701.696 | 566.147.112 | 238.204.041 |
| valores-grandes-carga | 536.870.912 | 671.088.964 | 285.802.432 | 538.972.160 | 625.836.331 | 336.042.541 |
| multivalor-carga | 33.554.432 | 536.871.236 | 54.071.232 | 67.375.104 | no aplica | no aplica |
| tamano-fijo-carga | 33.554.432 | 570.425.668 | 46.190.528 | 44.044.288 | no aplica | no aplica |

## Opciones cambiadas

- LMDB: tamaño del mapa de 1 GiB; claves duplicadas ordenadas en W19 y W20; claves enteras en W21 y W22.
- canopydb: confirmación que espera al disco del diario.
- fjall: caché de bloques de 1 GiB; llevar el diario al disco (SyncData) en cada confirmación.
