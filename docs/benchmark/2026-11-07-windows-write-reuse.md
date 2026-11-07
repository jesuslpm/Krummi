# Banco comparativo de Krummi

- Fecha: 2026-11-07 14:42 (UTC+00:00)
- Ejecución: completa
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: e0f33f8dc0f3 (con cambios sin confirmar)
- Motores: Voron 7.2.7 (.NET 10.0.5), LMDB 0.9.70 (heed 0.23.1), redb 4.4.0, canopydb 0.2.5, fjall 3.1.13
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 11 de 22 umbrales. Incumple: `carga-en-orden`, `carga-al-azar`, `intervalos`, `lecturas-con-escritor`, `escritor-con-lectores`, `borrado`, `reutilizacion`, `valores-grandes-carga`, `valores-grandes-lectura`, `multivalor-carga`, `tamano-fijo-carga`.

## Operaciones por segundo

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall | Krummi / Voron | Umbral |
|---|---|---|---|---|---|---|---|---|
| carga-en-orden | 491.646 | 680.590 | 164.104 | 178.281 | 220.718 | 176.261 | 0,72 | incumple |
| carga-al-azar | 41.227 | 102.563 | 30.362 | 18.809 | 125.427 | 151.827 | 0,40 | incumple |
| carga-masiva | 281.818 | 337.160 | 94.267 | 331.104 | 184.480 | 274.168 | 0,83 | cumple |
| escrituras-sueltas | 6.931 | 6.632 | 3.362 | 253 | 265 | 276 | 1,04 | cumple |
| escrituras-concurrentes-4 | 13.442 | 6.347 | 3.412 | 250 | 263 | 277 | 2,11 | cumple |
| escrituras-concurrentes-16 | 14.717 | 6.362 | 3.578 | 249 | 268 | 275 | 2,31 | cumple |
| lecturas-al-azar | 992.406 | 843.716 | 979.501 | 851.750 | 975.760 | 257.821 | 1,17 | cumple |
| lecturas-al-azar-4 | 4.084.265 | 3.394.749 | 4.249.614 | 2.525.395 | 2.771.717 | 907.416 | 1,20 | cumple |
| lecturas-al-azar-16 | 5.395.241 | 4.267.247 | 6.240.690 | 2.786.112 | 2.892.872 | 986.098 | 1,26 | cumple |
| recorrido | 15.997.901 | 8.915.286 | 13.194.840 | 7.981.846 | 7.121.817 | 2.584.921 | 1,79 | cumple |
| intervalos | 12.079.452 | 15.207.201 | 12.085.118 | 6.829.631 | 5.720.918 | 1.863.602 | 0,79 | incumple |
| lecturas-con-escritor | 877.562 | 2.322.684 | 1.959.115 | 2.025.009 | 1.910.626 | 795.915 | 0,37 | incumple |
| escritor-con-lectores | 32.097 | 67.821 | 10.948 | 6.858 | 18.722 | 24.632 | 0,47 | incumple |
| borrado | 47.857 | 101.137 | 30.090 | 16.585 | 38.463 | 160.970 | 0,47 | incumple |
| reutilizacion | 44.019 | 103.429 | 30.696 | 14.948 | 40.296 | 157.976 | 0,42 | incumple |
| apertura | 63 | 1 | 1.977 | 57 | 8 | 1 | 42,72 | cumple |
| valores-grandes-carga | 3.031 | 4.923 | 3.445 | 3.255 | 3.396 | 5.761 | 0,61 | incumple |
| valores-grandes-lectura | 51.625 | 109.852 | 60.534 | 50.072 | 58.878 | 54.935 | 0,46 | incumple |
| multivalor-carga | 44.722 | 118.772 | 43.214 | 25.062 | no aplica | no aplica | 0,37 | incumple |
| multivalor-lectura | 7.438.799 | 7.677.561 | 9.490.259 | 6.494.751 | no aplica | no aplica | 0,96 | cumple |
| tamano-fijo-carga | 96.650 | 148.047 | 46.578 | 27.286 | no aplica | no aplica | 0,65 | incumple |
| tamano-fijo-lectura | 2.194.838 | 1.815.204 | 1.335.564 | 1.520.289 | no aplica | no aplica | 1,20 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall |
|---|---|---|---|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 | 133.054.400 | 134.746.112 | 208.160.787 | 160.711.632 |
| carga-al-azar | 268.435.456 | 671.088.964 | 200.875.968 | 269.488.128 | 581.959.979 | 160.945.569 |
| carga-masiva | 268.435.456 | 402.719.044 | 189.239.232 | 269.488.128 | 344.170.216 | 191.723.778 |
| borrado | 268.435.456 | 671.088.964 | 201.248.704 | 585.728 | 557.818.155 | 203.692.607 |
| reutilizacion | 268.435.456 | 671.088.964 | 208.777.152 | 268.701.696 | 570.401.067 | 238.204.041 |
| valores-grandes-carga | 536.870.912 | 671.088.964 | 285.802.432 | 538.972.160 | 625.836.331 | 336.042.541 |
| multivalor-carga | 33.554.432 | 536.871.236 | 54.071.232 | 67.375.104 | no aplica | no aplica |
| tamano-fijo-carga | 33.554.432 | 570.425.668 | 46.190.528 | 44.044.288 | no aplica | no aplica |

## Opciones cambiadas

- LMDB: tamaño del mapa de 1 GiB; claves duplicadas ordenadas en W19 y W20; claves enteras en W21 y W22.
- canopydb: confirmación que espera al disco del diario.
- fjall: caché de bloques de 1 GiB; llevar el diario al disco (SyncData) en cada confirmación.
