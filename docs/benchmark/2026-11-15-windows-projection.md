# Banco comparativo de Krummi

- Fecha: 2026-11-15 18:36 (UTC+00:00)
- Ejecución: completa, solo Krummi y Voron
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: f4b6c2f94bae
- Motores: Voron 7.2.7 (.NET 10.0.5)
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 29 de 29 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 965.692 | 686.782 | 1,40 | cumple |
| carga-al-azar | 117.425 | 110.320 | 1,06 | cumple |
| carga-masiva | 420.065 | 361.858 | 1,16 | cumple |
| escrituras-sueltas | 8.386 | 7.377 | 1,13 | cumple |
| escrituras-concurrentes-4 | 19.720 | 7.272 | 2,71 | cumple |
| escrituras-concurrentes-16 | 23.634 | 7.336 | 3,22 | cumple |
| lecturas-al-azar | 971.695 | 881.984 | 1,10 | cumple |
| lecturas-al-azar-4 | 4.283.045 | 3.539.377 | 1,21 | cumple |
| lecturas-al-azar-16 | 4.889.808 | 4.219.813 | 1,15 | cumple |
| recorrido | 25.165.273 | 9.157.744 | 2,74 | cumple |
| intervalos | 17.424.562 | 16.960.021 | 1,02 | cumple |
| lecturas-con-escritor | 2.434.582 | 2.239.166 | 1,08 | cumple |
| escritor-con-lectores | 80.063 | 72.521 | 1,10 | cumple |
| borrado | 121.057 | 105.255 | 1,15 | cumple |
| reutilizacion | 114.527 | 106.711 | 1,07 | cumple |
| apertura | 67 | 2 | 43,90 | cumple |
| valores-grandes-carga | 6.569 | 5.534 | 1,18 | cumple |
| valores-grandes-lectura | 165.668 | 107.787 | 1,53 | cumple |
| multivalor-carga | 118.198 | 122.897 | 0,96 | cumple |
| multivalor-lectura | 8.366.920 | 7.773.952 | 1,07 | cumple |
| tamano-fijo-carga | 151.272 | 156.482 | 0,96 | cumple |
| tamano-fijo-lectura | 2.098.103 | 1.811.746 | 1,15 | cumple |
| tabla-insercion-en-orden | 452.532 | 337.265 | 1,34 | cumple |
| tabla-insercion-al-azar | 97.257 | 106.579 | 0,91 | cumple |
| tabla-carga-masiva | 465.002 | 436.474 | 1,06 | cumple |
| tabla-lecturas | 747.621 | 767.707 | 0,97 | cumple |
| tabla-intervalos | 10.274.458 | 10.638.713 | 0,96 | cumple |
| tabla-actualizacion | 115.008 | 130.948 | 0,87 | cumple |
| tabla-borrado | 120.534 | 53.500 | 2,25 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron |
|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 |
| carga-al-azar | 268.435.456 | 671.088.964 |
| carga-masiva | 268.435.456 | 402.719.044 |
| borrado | 268.435.456 | 671.088.964 |
| reutilizacion | 268.435.456 | 671.088.964 |
| valores-grandes-carga | 536.870.912 | 671.088.964 |
| multivalor-carga | 33.554.432 | 536.871.236 |
| tamano-fijo-carga | 33.554.432 | 570.425.668 |
| tabla-insercion-en-orden | 134.217.728 | 369.099.076 |
| tabla-insercion-al-azar | 268.435.456 | 671.088.964 |
| tabla-carga-masiva | 134.217.728 | 402.719.044 |
| tabla-borrado | 134.217.728 | 805.306.692 |

## Opciones cambiadas

Ninguna.
