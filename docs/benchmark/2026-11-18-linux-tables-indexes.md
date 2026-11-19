# Banco comparativo de Krummi

- Fecha: 2026-11-18 23:10 (UTC+00:00)
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

Krummi cumple 32 de 32 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 154.164 | 135.492 | 1,13 | cumple |
| carga-al-azar | 67.769 | 63.962 | 1,05 | cumple |
| carga-masiva | 355.583 | 292.506 | 1,21 | cumple |
| escrituras-sueltas | 182 | 182 | 0,99 | cumple |
| escrituras-concurrentes-4 | 396 | 181 | 2,19 | cumple |
| escrituras-concurrentes-16 | 1.420 | 179 | 7,92 | cumple |
| lecturas-al-azar | 865.366 | 788.127 | 1,09 | cumple |
| lecturas-al-azar-4 | 2.998.681 | 3.002.992 | 0,99 | cumple |
| lecturas-al-azar-16 | 4.680.988 | 3.938.621 | 1,18 | cumple |
| recorrido | 26.372.667 | 7.929.658 | 3,32 | cumple |
| intervalos | 19.377.243 | 14.526.575 | 1,33 | cumple |
| lecturas-con-escritor | 3.050.456 | 2.424.833 | 1,25 | cumple |
| escritor-con-lectores | 14.311 | 13.021 | 1,09 | cumple |
| borrado | 73.698 | 58.374 | 1,26 | cumple |
| reutilizacion | 68.865 | 59.787 | 1,15 | cumple |
| apertura | 478 | 3 | 169,23 | cumple |
| valores-grandes-carga | 4.512 | 3.424 | 1,31 | cumple |
| valores-grandes-lectura | 131.258 | 88.620 | 1,48 | cumple |
| multivalor-carga | 70.805 | 65.135 | 1,08 | cumple |
| multivalor-lectura | 8.908.226 | 7.249.715 | 1,22 | cumple |
| tamano-fijo-carga | 62.412 | 67.886 | 0,91 | cumple |
| tamano-fijo-lectura | 1.850.484 | 1.665.161 | 1,11 | cumple |
| tabla-insercion-en-orden | 129.761 | 116.105 | 1,11 | cumple |
| tabla-insercion-al-azar | 61.667 | 60.098 | 1,02 | cumple |
| tabla-carga-masiva | 492.471 | 407.330 | 1,20 | cumple |
| tabla-lecturas | 742.644 | 695.511 | 1,06 | cumple |
| tabla-intervalos | 11.227.210 | 10.363.374 | 1,08 | cumple |
| tabla-actualizacion | 77.845 | 71.650 | 1,08 | cumple |
| tabla-borrado | 73.034 | 38.453 | 1,89 | cumple |
| tabla-insercion-con-indices | 40.747 | 33.907 | 1,20 | cumple |
| tabla-busqueda-por-indice | 312.602 | 239.016 | 1,30 | cumple |
| tabla-intervalos-de-indice | 812.086 | 816.054 | 0,99 | cumple |

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
| tabla-insercion-con-indices | 268.435.456 | 1.207.959.876 |

## Opciones cambiadas

Ninguna.
