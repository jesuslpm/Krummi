# Banco comparativo de Krummi

- Fecha: 2026-11-12 21:57 (UTC+00:00)
- Ejecución: completa, solo Krummi y Voron
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: 6ffad001e78a
- Motores: Voron 7.2.7 (.NET 10.0.5)
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 29 de 29 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 946.885 | 671.725 | 1,40 | cumple |
| carga-al-azar | 115.471 | 109.998 | 1,04 | cumple |
| carga-masiva | 420.341 | 344.933 | 1,21 | cumple |
| escrituras-sueltas | 8.623 | 7.408 | 1,16 | cumple |
| escrituras-concurrentes-4 | 20.070 | 7.366 | 2,72 | cumple |
| escrituras-concurrentes-16 | 23.061 | 7.184 | 3,21 | cumple |
| lecturas-al-azar | 946.954 | 837.927 | 1,13 | cumple |
| lecturas-al-azar-4 | 4.296.074 | 3.559.715 | 1,20 | cumple |
| lecturas-al-azar-16 | 6.316.232 | 4.489.632 | 1,40 | cumple |
| recorrido | 17.721.447 | 9.396.845 | 1,88 | cumple |
| intervalos | 14.766.825 | 16.396.387 | 0,90 | cumple |
| lecturas-con-escritor | 2.625.483 | 2.316.056 | 1,13 | cumple |
| escritor-con-lectores | 89.138 | 73.125 | 1,21 | cumple |
| borrado | 121.275 | 103.440 | 1,17 | cumple |
| reutilizacion | 114.460 | 104.907 | 1,09 | cumple |
| apertura | 64 | 2 | 41,97 | cumple |
| valores-grandes-carga | 5.211 | 5.650 | 0,92 | cumple |
| valores-grandes-lectura | 150.710 | 104.339 | 1,44 | cumple |
| multivalor-carga | 117.792 | 119.866 | 0,98 | cumple |
| multivalor-lectura | 8.374.480 | 7.504.217 | 1,11 | cumple |
| tamano-fijo-carga | 151.747 | 150.036 | 1,01 | cumple |
| tamano-fijo-lectura | 2.079.312 | 1.833.539 | 1,13 | cumple |
| tabla-insercion-en-orden | 459.250 | 330.385 | 1,39 | cumple |
| tabla-insercion-al-azar | 96.546 | 104.653 | 0,92 | cumple |
| tabla-carga-masiva | 457.500 | 435.599 | 1,05 | cumple |
| tabla-lecturas | 741.549 | 730.477 | 1,01 | cumple |
| tabla-intervalos | 9.060.425 | 10.491.632 | 0,86 | cumple |
| tabla-actualizacion | 113.536 | 128.264 | 0,88 | cumple |
| tabla-borrado | 120.371 | 52.916 | 2,27 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron |
|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 |
| carga-al-azar | 268.435.456 | 671.088.964 |
| carga-masiva | 268.435.456 | 402.719.044 |
| borrado | 268.435.456 | 671.088.964 |
| reutilizacion | 268.435.456 | 671.088.964 |
| valores-grandes-carga | 536.870.912 | 738.197.828 |
| multivalor-carga | 33.554.432 | 402.653.508 |
| tamano-fijo-carga | 33.554.432 | 570.425.668 |
| tabla-insercion-en-orden | 134.217.728 | 369.099.076 |
| tabla-insercion-al-azar | 268.435.456 | 671.088.964 |
| tabla-carga-masiva | 134.217.728 | 402.719.044 |
| tabla-borrado | 134.217.728 | 805.306.692 |

## Opciones cambiadas

Ninguna.
