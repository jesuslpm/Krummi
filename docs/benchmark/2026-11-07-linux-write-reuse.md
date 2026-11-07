# Banco comparativo de Krummi

- Fecha: 2026-11-07 16:19 (UTC+00:00)
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

Krummi cumple 14 de 22 umbrales. Incumple: `carga-al-azar`, `intervalos`, `lecturas-con-escritor`, `borrado`, `reutilizacion`, `valores-grandes-lectura`, `multivalor-carga`, `tamano-fijo-carga`.

## Operaciones por segundo

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall | Krummi / Voron | Umbral |
|---|---|---|---|---|---|---|---|---|
| carga-en-orden | 92.528 | 95.933 | 72.230 | 90.807 | 97.315 | 88.715 | 0,96 | cumple |
| carga-al-azar | 24.130 | 49.997 | 27.795 | 23.442 | 66.109 | 80.040 | 0,48 | incumple |
| carga-masiva | 255.006 | 308.617 | 489.732 | 335.364 | 315.365 | 229.665 | 0,82 | cumple |
| escrituras-sueltas | 119 | 120 | 123 | 227 | 222 | 122 | 0,99 | cumple |
| escrituras-concurrentes-4 | 250 | 121 | 120 | 225 | 223 | 116 | 2,06 | cumple |
| escrituras-concurrentes-16 | 993 | 119 | 121 | 228 | 221 | 115 | 8,38 | cumple |
| lecturas-al-azar | 761.761 | 733.047 | 888.269 | 730.191 | 900.262 | 239.497 | 1,03 | cumple |
| lecturas-al-azar-4 | 3.450.582 | 3.166.177 | 3.757.574 | 2.288.902 | 2.714.479 | 811.367 | 1,08 | cumple |
| lecturas-al-azar-16 | 5.081.278 | 4.364.110 | 4.894.737 | 2.372.771 | 1.946.321 | 769.910 | 1,16 | cumple |
| recorrido | 13.793.120 | 10.933.848 | 13.839.803 | 11.685.176 | 6.875.717 | 2.416.094 | 1,26 | cumple |
| intervalos | 11.522.063 | 15.103.496 | 12.389.766 | 9.250.920 | 5.744.798 | 1.750.940 | 0,76 | incumple |
| lecturas-con-escritor | 1.671.754 | 2.642.389 | 3.301.433 | 2.146.324 | 2.369.957 | 749.599 | 0,63 | incumple |
| escritor-con-lectores | 7.570 | 9.240 | 6.752 | 10.072 | 9.676 | 11.129 | 0,81 | cumple |
| borrado | 27.147 | 48.500 | 24.958 | 25.795 | 58.917 | 83.659 | 0,55 | incumple |
| reutilizacion | 25.329 | 48.224 | 24.745 | 22.761 | 56.614 | 80.970 | 0,52 | incumple |
| apertura | 306 | 3 | 6.222 | 54 | 9 | 1 | 119,25 | cumple |
| valores-grandes-carga | 2.565 | 2.412 | 4.443 | 2.280 | 2.825 | 3.650 | 1,06 | cumple |
| valores-grandes-lectura | 51.471 | 99.467 | 58.701 | 54.479 | 56.748 | 46.884 | 0,51 | incumple |
| multivalor-carga | 27.652 | 50.100 | 33.787 | 33.025 | no aplica | no aplica | 0,55 | incumple |
| multivalor-lectura | 7.794.677 | 6.568.980 | 9.142.925 | 6.456.869 | no aplica | no aplica | 1,18 | cumple |
| tamano-fijo-carga | 37.482 | 52.161 | 39.260 | 34.364 | no aplica | no aplica | 0,71 | incumple |
| tamano-fijo-lectura | 1.992.776 | 1.664.411 | 1.488.805 | 1.434.777 | no aplica | no aplica | 1,19 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall |
|---|---|---|---|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 | 133.054.464 | 134.746.112 | 139.309.355 | 160.711.632 |
| carga-al-azar | 268.435.456 | 671.088.964 | 200.876.032 | 269.488.128 | 345.067.819 | 160.945.569 |
| carga-masiva | 268.435.456 | 402.719.044 | 189.239.296 | 269.488.128 | 344.170.216 | 191.723.778 |
| borrado | 268.435.456 | 671.088.964 | 201.248.768 | 585.728 | 361.713.963 | 203.692.607 |
| reutilizacion | 268.435.456 | 671.088.964 | 208.777.216 | 268.701.696 | 379.023.659 | 238.204.041 |
| valores-grandes-carga | 536.870.912 | 671.088.964 | 285.802.496 | 538.972.160 | 625.836.331 | 269.111.157 |
| multivalor-carga | 33.554.432 | 335.544.644 | 54.071.296 | 67.375.104 | no aplica | no aplica |
| tamano-fijo-carga | 33.554.432 | 570.425.668 | 46.190.592 | 44.044.288 | no aplica | no aplica |

## Opciones cambiadas

- LMDB: tamaño del mapa de 1 GiB; claves duplicadas ordenadas en W19 y W20; claves enteras en W21 y W22.
- canopydb: confirmación que espera al disco del diario.
- fjall: caché de bloques de 1 GiB; llevar el diario al disco (SyncData) en cada confirmación.
