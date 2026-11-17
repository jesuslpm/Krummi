# Banco comparativo de Krummi

- Fecha: 2026-11-17 00:54 (UTC+00:00)
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

Krummi cumple 29 de 29 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 157.723 | 126.553 | 1,24 | cumple |
| carga-al-azar | 64.332 | 49.758 | 1,29 | cumple |
| carga-masiva | 375.575 | 297.792 | 1,26 | cumple |
| escrituras-sueltas | 198 | 196 | 1,01 | cumple |
| escrituras-concurrentes-4 | 410 | 194 | 2,11 | cumple |
| escrituras-concurrentes-16 | 1.600 | 195 | 8,20 | cumple |
| lecturas-al-azar | 806.724 | 766.361 | 1,05 | cumple |
| lecturas-al-azar-4 | 3.545.172 | 3.169.833 | 1,11 | cumple |
| lecturas-al-azar-16 | 5.061.619 | 4.099.413 | 1,23 | cumple |
| recorrido | 30.241.010 | 19.963.270 | 1,51 | cumple |
| intervalos | 18.906.503 | 14.878.393 | 1,27 | cumple |
| lecturas-con-escritor | 2.970.092 | 2.625.155 | 1,13 | cumple |
| escritor-con-lectores | 14.666 | 13.527 | 1,08 | cumple |
| borrado | 70.218 | 58.120 | 1,20 | cumple |
| reutilizacion | 66.729 | 58.623 | 1,13 | cumple |
| apertura | 490 | 3 | 177,74 | cumple |
| valores-grandes-carga | 5.035 | 3.303 | 1,52 | cumple |
| valores-grandes-lectura | 145.102 | 100.528 | 1,44 | cumple |
| multivalor-carga | 68.263 | 61.873 | 1,10 | cumple |
| multivalor-lectura | 9.014.120 | 6.799.156 | 1,32 | cumple |
| tamano-fijo-carga | 58.242 | 63.940 | 0,91 | cumple |
| tamano-fijo-lectura | 1.884.939 | 1.609.336 | 1,17 | cumple |
| tabla-insercion-en-orden | 95.876 | 118.469 | 0,80 | cumple |
| tabla-insercion-al-azar | 48.409 | 57.291 | 0,84 | cumple |
| tabla-carga-masiva | 434.625 | 326.894 | 1,32 | cumple |
| tabla-lecturas | 704.944 | 674.267 | 1,04 | cumple |
| tabla-intervalos | 12.172.450 | 10.283.417 | 1,18 | cumple |
| tabla-actualizacion | 66.016 | 71.274 | 0,92 | cumple |
| tabla-borrado | 71.965 | 37.953 | 1,89 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron |
|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 |
| carga-al-azar | 268.435.456 | 671.088.964 |
| carga-masiva | 268.435.456 | 402.719.044 |
| borrado | 268.435.456 | 671.088.964 |
| reutilizacion | 268.435.456 | 671.088.964 |
| valores-grandes-carga | 536.870.912 | 738.197.828 |
| multivalor-carga | 33.554.432 | 335.544.644 |
| tamano-fijo-carga | 33.554.432 | 570.425.668 |
| tabla-insercion-en-orden | 134.217.728 | 369.099.076 |
| tabla-insercion-al-azar | 268.435.456 | 671.088.964 |
| tabla-carga-masiva | 134.217.728 | 402.719.044 |
| tabla-borrado | 134.217.728 | 671.088.964 |

## Opciones cambiadas

Ninguna.
