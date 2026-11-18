# Banco comparativo de Krummi

- Fecha: 2026-11-18 12:29 (UTC+00:00)
- Ejecución: completa, solo Krummi y Voron
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: 605d0ff11a2a (con cambios sin confirmar)
- Motores: Voron 7.2.7 (.NET 10.0.5)
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 29 de 29 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 1.295.570 | 677.189 | 1,91 | cumple |
| carga-al-azar | 118.065 | 109.996 | 1,07 | cumple |
| carga-masiva | 394.666 | 362.185 | 1,08 | cumple |
| escrituras-sueltas | 7.523 | 7.356 | 1,02 | cumple |
| escrituras-concurrentes-4 | 18.654 | 7.396 | 2,52 | cumple |
| escrituras-concurrentes-16 | 22.076 | 7.415 | 2,97 | cumple |
| lecturas-al-azar | 1.008.557 | 910.532 | 1,10 | cumple |
| lecturas-al-azar-4 | 3.980.682 | 3.180.527 | 1,25 | cumple |
| lecturas-al-azar-16 | 4.131.558 | 3.483.651 | 1,18 | cumple |
| recorrido | 32.247.038 | 11.390.118 | 2,83 | cumple |
| intervalos | 20.756.311 | 16.780.687 | 1,23 | cumple |
| lecturas-con-escritor | 1.433.150 | 1.441.159 | 0,99 | cumple |
| escritor-con-lectores | 48.504 | 43.418 | 1,11 | cumple |
| borrado | 123.463 | 102.350 | 1,20 | cumple |
| reutilizacion | 115.832 | 104.817 | 1,10 | cumple |
| apertura | 59 | 2 | 38,47 | cumple |
| valores-grandes-carga | 7.599 | 5.521 | 1,37 | cumple |
| valores-grandes-lectura | 165.189 | 111.902 | 1,47 | cumple |
| multivalor-carga | 131.925 | 122.010 | 1,08 | cumple |
| multivalor-lectura | 8.624.526 | 7.817.893 | 1,10 | cumple |
| tamano-fijo-carga | 156.389 | 152.136 | 1,02 | cumple |
| tamano-fijo-lectura | 2.148.897 | 1.894.992 | 1,13 | cumple |
| tabla-insercion-en-orden | 477.658 | 329.328 | 1,45 | cumple |
| tabla-insercion-al-azar | 99.822 | 104.801 | 0,95 | cumple |
| tabla-carga-masiva | 469.529 | 419.409 | 1,11 | cumple |
| tabla-lecturas | 767.779 | 700.715 | 1,09 | cumple |
| tabla-intervalos | 11.889.565 | 10.613.863 | 1,12 | cumple |
| tabla-actualizacion | 128.206 | 122.866 | 1,04 | cumple |
| tabla-borrado | 122.347 | 52.112 | 2,34 | cumple |

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
