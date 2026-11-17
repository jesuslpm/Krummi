# Banco comparativo de Krummi

- Fecha: 2026-11-17 11:15 (UTC+00:00)
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
| carga-en-orden | 159.664 | 149.828 | 1,06 | cumple |
| carga-al-azar | 64.463 | 61.102 | 1,05 | cumple |
| carga-masiva | 385.492 | 323.044 | 1,19 | cumple |
| escrituras-sueltas | 195 | 194 | 1,00 | cumple |
| escrituras-concurrentes-4 | 415 | 196 | 2,11 | cumple |
| escrituras-concurrentes-16 | 1.602 | 193 | 8,27 | cumple |
| lecturas-al-azar | 809.976 | 768.338 | 1,05 | cumple |
| lecturas-al-azar-4 | 3.510.042 | 3.102.024 | 1,13 | cumple |
| lecturas-al-azar-16 | 4.316.460 | 3.664.663 | 1,17 | cumple |
| recorrido | 27.463.417 | 9.929.859 | 2,76 | cumple |
| intervalos | 19.086.137 | 14.957.251 | 1,27 | cumple |
| lecturas-con-escritor | 3.064.127 | 2.566.550 | 1,19 | cumple |
| escritor-con-lectores | 14.971 | 13.786 | 1,08 | cumple |
| borrado | 68.922 | 58.259 | 1,18 | cumple |
| reutilizacion | 67.743 | 59.500 | 1,13 | cumple |
| apertura | 357 | 3 | 129,78 | cumple |
| valores-grandes-carga | 3.339 | 3.123 | 1,06 | cumple |
| valores-grandes-lectura | 147.299 | 105.801 | 1,39 | cumple |
| multivalor-carga | 69.163 | 62.500 | 1,10 | cumple |
| multivalor-lectura | 9.040.807 | 6.690.107 | 1,35 | cumple |
| tamano-fijo-carga | 60.651 | 66.672 | 0,90 | cumple |
| tamano-fijo-lectura | 1.988.885 | 1.649.907 | 1,20 | cumple |
| tabla-insercion-en-orden | 136.967 | 120.353 | 1,13 | cumple |
| tabla-insercion-al-azar | 60.005 | 57.175 | 1,04 | cumple |
| tabla-carga-masiva | 446.057 | 401.959 | 1,10 | cumple |
| tabla-lecturas | 699.058 | 682.920 | 1,02 | cumple |
| tabla-intervalos | 12.205.851 | 10.275.451 | 1,18 | cumple |
| tabla-actualizacion | 75.622 | 68.786 | 1,09 | cumple |
| tabla-borrado | 69.778 | 38.255 | 1,82 | cumple |

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
