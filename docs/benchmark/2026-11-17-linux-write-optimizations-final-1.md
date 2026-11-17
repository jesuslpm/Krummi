# Banco comparativo de Krummi

- Fecha: 2026-11-17 10:04 (UTC+00:00)
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

Krummi cumple 28 de 29 umbrales. Incumple: `lecturas-al-azar-16`.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 164.165 | 123.768 | 1,32 | cumple |
| carga-al-azar | 65.796 | 57.928 | 1,13 | cumple |
| carga-masiva | 378.211 | 299.481 | 1,26 | cumple |
| escrituras-sueltas | 190 | 189 | 1,00 | cumple |
| escrituras-concurrentes-4 | 360 | 192 | 1,86 | cumple |
| escrituras-concurrentes-16 | 1.436 | 178 | 8,05 | cumple |
| lecturas-al-azar | 822.359 | 733.166 | 1,12 | cumple |
| lecturas-al-azar-4 | 2.665.616 | 2.716.784 | 0,98 | cumple |
| lecturas-al-azar-16 | 2.155.460 | 3.130.048 | 0,68 | incumple |
| recorrido | 22.371.090 | 10.352.778 | 2,16 | cumple |
| intervalos | 19.845.970 | 15.484.655 | 1,28 | cumple |
| lecturas-con-escritor | 2.758.603 | 2.633.485 | 1,04 | cumple |
| escritor-con-lectores | 12.722 | 13.050 | 0,97 | cumple |
| borrado | 57.106 | 58.419 | 0,97 | cumple |
| reutilizacion | 67.358 | 57.238 | 1,17 | cumple |
| apertura | 462 | 3 | 167,80 | cumple |
| valores-grandes-carga | 4.334 | 3.524 | 1,22 | cumple |
| valores-grandes-lectura | 129.864 | 105.677 | 1,22 | cumple |
| multivalor-carga | 68.185 | 63.587 | 1,07 | cumple |
| multivalor-lectura | 8.486.964 | 6.775.639 | 1,25 | cumple |
| tamano-fijo-carga | 63.719 | 65.670 | 0,97 | cumple |
| tamano-fijo-lectura | 1.959.088 | 1.646.742 | 1,18 | cumple |
| tabla-insercion-en-orden | 138.403 | 117.844 | 1,17 | cumple |
| tabla-insercion-al-azar | 60.889 | 58.412 | 1,04 | cumple |
| tabla-carga-masiva | 488.147 | 400.015 | 1,22 | cumple |
| tabla-lecturas | 658.019 | 676.368 | 0,97 | cumple |
| tabla-intervalos | 11.603.692 | 10.065.125 | 1,15 | cumple |
| tabla-actualizacion | 74.489 | 68.292 | 1,09 | cumple |
| tabla-borrado | 71.505 | 35.645 | 2,00 | cumple |

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
