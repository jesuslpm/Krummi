# Banco comparativo de Krummi

- Fecha: 2026-11-16 16:08 (UTC+00:00)
- Ejecución: completa, solo Krummi y Voron
- Sistema: Ubuntu 24.04 (Linux 5.15.133.1-microsoft-standard-WSL2)
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Núcleos: sin fijar (el sistema no distingue núcleos de rendimiento)
- Memoria: 31 GiB
- Disco: Virtual Disk, ext4
- Krummi: desconocido
- Motores: Voron 7.2.7 (.NET 10.0.14)
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 28 de 29 umbrales. Incumple: `tamano-fijo-carga`.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 127.720 | 90.862 | 1,40 | cumple |
| carga-al-azar | 69.031 | 67.724 | 1,01 | cumple |
| carga-masiva | 717.802 | 520.160 | 1,37 | cumple |
| escrituras-sueltas | 109 | 111 | 0,98 | cumple |
| escrituras-concurrentes-4 | 231 | 110 | 2,09 | cumple |
| escrituras-concurrentes-16 | 1.003 | 111 | 9,03 | cumple |
| lecturas-al-azar | 1.457.653 | 1.355.738 | 1,07 | cumple |
| lecturas-al-azar-4 | 3.725.358 | 3.521.564 | 1,05 | cumple |
| lecturas-al-azar-16 | 4.686.561 | 4.153.213 | 1,12 | cumple |
| recorrido | 39.506.009 | 15.677.785 | 2,51 | cumple |
| intervalos | 25.223.058 | 23.407.796 | 1,07 | cumple |
| lecturas-con-escritor | 3.191.440 | 2.592.442 | 1,23 | cumple |
| escritor-con-lectores | 15.401 | 13.490 | 1,14 | cumple |
| borrado | 70.885 | 66.300 | 1,06 | cumple |
| reutilizacion | 69.740 | 68.284 | 1,02 | cumple |
| apertura | 448 | 4 | 106,81 | cumple |
| valores-grandes-carga | 5.057 | 4.330 | 1,16 | cumple |
| valores-grandes-lectura | 231.737 | 108.793 | 2,13 | cumple |
| multivalor-carga | 58.886 | 68.603 | 0,85 | cumple |
| multivalor-lectura | 16.510.345 | 8.385.441 | 1,96 | cumple |
| tamano-fijo-carga | 48.146 | 67.633 | 0,71 | incumple |
| tamano-fijo-lectura | 3.418.728 | 2.894.631 | 1,18 | cumple |
| tabla-insercion-en-orden | 80.663 | 84.540 | 0,95 | cumple |
| tabla-insercion-al-azar | 62.879 | 64.590 | 0,97 | cumple |
| tabla-carga-masiva | 864.651 | 641.042 | 1,34 | cumple |
| tabla-lecturas | 964.643 | 1.142.750 | 0,84 | cumple |
| tabla-intervalos | 17.393.443 | 16.003.810 | 1,08 | cumple |
| tabla-actualizacion | 72.893 | 75.746 | 0,96 | cumple |
| tabla-borrado | 70.517 | 54.113 | 1,30 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron |
|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 |
| carga-al-azar | 268.435.456 | 671.088.964 |
| carga-masiva | 268.435.456 | 402.719.044 |
| borrado | 268.435.456 | 671.088.964 |
| reutilizacion | 268.435.456 | 671.088.964 |
| valores-grandes-carga | 536.870.912 | 671.088.964 |
| multivalor-carga | 33.554.432 | 335.544.644 |
| tamano-fijo-carga | 33.554.432 | 570.425.668 |
| tabla-insercion-en-orden | 134.217.728 | 369.099.076 |
| tabla-insercion-al-azar | 268.435.456 | 671.088.964 |
| tabla-carga-masiva | 134.217.728 | 402.719.044 |
| tabla-borrado | 134.217.728 | 671.088.964 |

## Opciones cambiadas

Ninguna.
