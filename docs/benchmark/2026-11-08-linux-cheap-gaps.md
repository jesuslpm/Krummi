# Banco comparativo de Krummi

- Fecha: 2026-11-08 17:07 (UTC+00:00)
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

Krummi cumple 21 de 22 umbrales. Incumple: `carga-masiva`.

## Operaciones por segundo

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall | Krummi / Voron | Umbral |
|---|---|---|---|---|---|---|---|---|
| carga-en-orden | 101.204 | 95.071 | 72.003 | 95.742 | 99.038 | 93.658 | 1,06 | cumple |
| carga-al-azar | 53.385 | 51.610 | 24.205 | 23.441 | 66.896 | 78.876 | 1,03 | cumple |
| carga-masiva | 247.077 | 314.808 | 539.273 | 338.553 | 312.158 | 242.148 | 0,78 | incumple |
| escrituras-sueltas | 117 | 120 | 120 | 226 | 225 | 116 | 0,97 | cumple |
| escrituras-concurrentes-4 | 240 | 118 | 128 | 230 | 225 | 119 | 2,03 | cumple |
| escrituras-concurrentes-16 | 987 | 118 | 128 | 232 | 237 | 120 | 8,34 | cumple |
| lecturas-al-azar | 802.226 | 778.697 | 803.838 | 730.200 | 898.012 | 238.659 | 1,03 | cumple |
| lecturas-al-azar-4 | 3.503.593 | 3.282.309 | 3.636.548 | 2.314.563 | 2.742.501 | 834.857 | 1,06 | cumple |
| lecturas-al-azar-16 | 4.474.912 | 3.943.916 | 4.165.160 | 2.553.307 | 2.530.147 | 846.149 | 1,13 | cumple |
| recorrido | 16.462.379 | 20.500.705 | 10.923.529 | 11.777.239 | 6.677.011 | 2.553.842 | 0,80 | cumple |
| intervalos | 13.207.350 | 15.207.279 | 12.770.852 | 9.450.899 | 5.986.766 | 1.804.777 | 0,86 | cumple |
| lecturas-con-escritor | 2.303.667 | 2.681.986 | 2.944.268 | 2.250.054 | 2.410.799 | 788.060 | 0,85 | cumple |
| escritor-con-lectores | 10.696 | 9.758 | 6.435 | 10.341 | 9.833 | 10.987 | 1,09 | cumple |
| borrado | 55.651 | 48.860 | 25.920 | 25.344 | 56.791 | 85.491 | 1,13 | cumple |
| reutilizacion | 53.700 | 47.883 | 22.867 | 20.774 | 55.936 | 80.136 | 1,12 | cumple |
| apertura | 271 | 3 | 6.922 | 52 | 11 | 1 | 105,37 | cumple |
| valores-grandes-carga | 3.019 | 2.502 | 4.720 | 2.379 | 3.394 | 3.577 | 1,20 | cumple |
| valores-grandes-lectura | 151.305 | 93.094 | 162.439 | 121.706 | 150.453 | 118.551 | 1,62 | cumple |
| multivalor-carga | 50.101 | 51.557 | 33.043 | 33.036 | no aplica | no aplica | 0,97 | cumple |
| multivalor-lectura | 8.437.567 | 6.234.479 | 9.164.528 | 6.288.909 | no aplica | no aplica | 1,35 | cumple |
| tamano-fijo-carga | 43.781 | 52.876 | 39.530 | 33.536 | no aplica | no aplica | 0,82 | cumple |
| tamano-fijo-lectura | 1.949.785 | 1.646.035 | 1.510.856 | 1.416.710 | no aplica | no aplica | 1,18 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall |
|---|---|---|---|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 | 133.054.464 | 134.746.112 | 139.309.355 | 160.711.632 |
| carga-al-azar | 268.435.456 | 671.088.964 | 200.876.032 | 269.488.128 | 344.158.507 | 160.945.569 |
| carga-masiva | 268.435.456 | 402.719.044 | 189.239.296 | 269.488.128 | 344.170.216 | 191.723.778 |
| borrado | 268.435.456 | 671.088.964 | 201.248.768 | 585.728 | 361.496.875 | 203.692.607 |
| reutilizacion | 268.435.456 | 671.088.964 | 208.777.216 | 268.701.696 | 384.327.979 | 238.204.041 |
| valores-grandes-carga | 536.870.912 | 671.088.964 | 285.802.496 | 538.972.160 | 625.836.331 | 336.042.541 |
| multivalor-carga | 33.554.432 | 335.544.644 | 54.071.296 | 67.375.104 | no aplica | no aplica |
| tamano-fijo-carga | 33.554.432 | 570.425.668 | 46.190.592 | 44.044.288 | no aplica | no aplica |

## Opciones cambiadas

- LMDB: tamaño del mapa de 1 GiB; claves duplicadas ordenadas en W19 y W20; claves enteras en W21 y W22.
- canopydb: confirmación que espera al disco del diario.
- fjall: caché de bloques de 1 GiB; llevar el diario al disco (SyncData) en cada confirmación.
