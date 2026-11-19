# Banco comparativo de Krummi

- Fecha: 2026-11-18 22:44 (UTC+00:00)
- Ejecución: completa, solo Krummi y Voron
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: 1de443b3e7df (con cambios sin confirmar)
- Motores: Voron 7.2.7 (.NET 10.0.5)
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 32 de 32 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 1.398.728 | 712.208 | 1,96 | cumple |
| carga-al-azar | 119.074 | 112.055 | 1,06 | cumple |
| carga-masiva | 424.704 | 370.558 | 1,14 | cumple |
| escrituras-sueltas | 8.320 | 7.378 | 1,12 | cumple |
| escrituras-concurrentes-4 | 20.055 | 7.164 | 2,79 | cumple |
| escrituras-concurrentes-16 | 23.548 | 7.076 | 3,32 | cumple |
| lecturas-al-azar | 1.048.791 | 895.903 | 1,17 | cumple |
| lecturas-al-azar-4 | 3.410.143 | 3.487.622 | 0,97 | cumple |
| lecturas-al-azar-16 | 5.125.405 | 4.268.908 | 1,20 | cumple |
| recorrido | 32.635.798 | 10.591.425 | 3,08 | cumple |
| intervalos | 21.097.432 | 17.009.802 | 1,24 | cumple |
| lecturas-con-escritor | 2.589.279 | 2.271.712 | 1,13 | cumple |
| escritor-con-lectores | 87.626 | 72.354 | 1,21 | cumple |
| borrado | 124.994 | 103.749 | 1,20 | cumple |
| reutilizacion | 117.584 | 107.809 | 1,09 | cumple |
| apertura | 66 | 2 | 43,00 | cumple |
| valores-grandes-carga | 7.812 | 5.536 | 1,41 | cumple |
| valores-grandes-lectura | 164.168 | 107.065 | 1,53 | cumple |
| multivalor-carga | 134.958 | 123.097 | 1,09 | cumple |
| multivalor-lectura | 8.640.757 | 7.774.061 | 1,11 | cumple |
| tamano-fijo-carga | 157.988 | 157.160 | 1,00 | cumple |
| tamano-fijo-lectura | 2.182.299 | 1.877.219 | 1,16 | cumple |
| tabla-insercion-en-orden | 474.478 | 340.647 | 1,39 | cumple |
| tabla-insercion-al-azar | 98.412 | 107.244 | 0,91 | cumple |
| tabla-carga-masiva | 470.043 | 436.982 | 1,07 | cumple |
| tabla-lecturas | 755.192 | 801.626 | 0,94 | cumple |
| tabla-intervalos | 11.014.740 | 11.012.186 | 1,00 | cumple |
| tabla-actualizacion | 127.179 | 132.251 | 0,96 | cumple |
| tabla-borrado | 123.388 | 54.172 | 2,27 | cumple |
| tabla-insercion-con-indices | 54.643 | 50.642 | 1,07 | cumple |
| tabla-busqueda-por-indice | 290.309 | 272.357 | 1,06 | cumple |
| tabla-intervalos-de-indice | 953.346 | 963.922 | 0,98 | cumple |

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
| tabla-insercion-con-indices | 268.435.456 | 1.073.742.148 |

## Opciones cambiadas

Ninguna.
