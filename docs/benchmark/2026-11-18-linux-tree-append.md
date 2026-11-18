# Banco comparativo de Krummi

- Fecha: 2026-11-18 12:52 (UTC+00:00)
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
| carga-en-orden | 150.562 | 139.249 | 1,08 | cumple |
| carga-al-azar | 64.406 | 60.556 | 1,06 | cumple |
| carga-masiva | 359.865 | 313.869 | 1,14 | cumple |
| escrituras-sueltas | 179 | 181 | 0,99 | cumple |
| escrituras-concurrentes-4 | 383 | 181 | 2,11 | cumple |
| escrituras-concurrentes-16 | 1.558 | 181 | 8,63 | cumple |
| lecturas-al-azar | 789.869 | 801.043 | 0,98 | cumple |
| lecturas-al-azar-4 | 3.472.727 | 2.748.882 | 1,26 | cumple |
| lecturas-al-azar-16 | 2.864.056 | 3.380.404 | 0,84 | cumple |
| recorrido | 29.544.122 | 12.111.980 | 2,43 | cumple |
| intervalos | 17.207.097 | 15.762.208 | 1,09 | cumple |
| lecturas-con-escritor | 2.767.598 | 2.305.099 | 1,20 | cumple |
| escritor-con-lectores | 14.110 | 12.103 | 1,16 | cumple |
| borrado | 69.854 | 58.515 | 1,19 | cumple |
| reutilizacion | 70.547 | 54.325 | 1,29 | cumple |
| apertura | 416 | 3 | 148,08 | cumple |
| valores-grandes-carga | 4.349 | 2.785 | 1,56 | cumple |
| valores-grandes-lectura | 136.343 | 96.007 | 1,42 | cumple |
| multivalor-carga | 69.983 | 62.208 | 1,12 | cumple |
| multivalor-lectura | 8.861.213 | 6.492.081 | 1,36 | cumple |
| tamano-fijo-carga | 58.302 | 63.212 | 0,92 | cumple |
| tamano-fijo-lectura | 1.902.218 | 1.498.340 | 1,26 | cumple |
| tabla-insercion-en-orden | 124.621 | 106.705 | 1,16 | cumple |
| tabla-insercion-al-azar | 60.314 | 57.247 | 1,05 | cumple |
| tabla-carga-masiva | 480.547 | 368.777 | 1,30 | cumple |
| tabla-lecturas | 735.928 | 695.636 | 1,05 | cumple |
| tabla-intervalos | 12.288.622 | 10.105.549 | 1,21 | cumple |
| tabla-actualizacion | 79.124 | 69.873 | 1,13 | cumple |
| tabla-borrado | 73.902 | 36.783 | 2,00 | cumple |

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
