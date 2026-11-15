# Banco comparativo de Krummi

- Fecha: 2026-11-15 18:58 (UTC+00:00)
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
| carga-en-orden | 156.518 | 140.402 | 1,11 | cumple |
| carga-al-azar | 63.614 | 60.594 | 1,04 | cumple |
| carga-masiva | 372.200 | 317.305 | 1,17 | cumple |
| escrituras-sueltas | 197 | 191 | 1,03 | cumple |
| escrituras-concurrentes-4 | 403 | 195 | 2,06 | cumple |
| escrituras-concurrentes-16 | 1.672 | 192 | 8,72 | cumple |
| lecturas-al-azar | 844.351 | 757.180 | 1,11 | cumple |
| lecturas-al-azar-4 | 3.455.079 | 3.014.064 | 1,14 | cumple |
| lecturas-al-azar-16 | 4.371.070 | 3.743.826 | 1,16 | cumple |
| recorrido | 22.917.741 | 10.257.744 | 2,23 | cumple |
| intervalos | 15.761.804 | 15.015.695 | 1,04 | cumple |
| lecturas-con-escritor | 2.941.734 | 2.597.083 | 1,13 | cumple |
| escritor-con-lectores | 15.373 | 13.298 | 1,15 | cumple |
| borrado | 69.540 | 57.777 | 1,20 | cumple |
| reutilizacion | 67.272 | 58.274 | 1,15 | cumple |
| apertura | 337 | 3 | 127,83 | cumple |
| valores-grandes-carga | 3.783 | 2.870 | 1,31 | cumple |
| valores-grandes-lectura | 142.492 | 98.706 | 1,44 | cumple |
| multivalor-carga | 64.270 | 61.424 | 1,04 | cumple |
| multivalor-lectura | 8.573.257 | 6.501.035 | 1,31 | cumple |
| tamano-fijo-carga | 56.276 | 63.469 | 0,88 | cumple |
| tamano-fijo-lectura | 1.861.350 | 1.683.845 | 1,10 | cumple |
| tabla-insercion-en-orden | 139.456 | 113.441 | 1,22 | cumple |
| tabla-insercion-al-azar | 58.765 | 57.153 | 1,02 | cumple |
| tabla-carga-masiva | 492.075 | 371.061 | 1,32 | cumple |
| tabla-lecturas | 662.955 | 679.994 | 0,97 | cumple |
| tabla-intervalos | 10.364.726 | 9.900.016 | 1,04 | cumple |
| tabla-actualizacion | 69.724 | 69.366 | 1,00 | cumple |
| tabla-borrado | 69.969 | 37.833 | 1,84 | cumple |

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
