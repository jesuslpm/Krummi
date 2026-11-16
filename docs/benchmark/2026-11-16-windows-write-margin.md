# Banco comparativo de Krummi

- Fecha: 2026-11-16 13:36 (UTC+00:00)
- Ejecución: completa, solo Krummi y Voron
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: e05655c82301 (con cambios sin confirmar)
- Motores: Voron 7.2.7 (.NET 10.0.5)
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 29 de 29 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 877.621 | 656.705 | 1,33 | cumple |
| carga-al-azar | 111.987 | 102.635 | 1,09 | cumple |
| carga-masiva | 398.059 | 345.603 | 1,15 | cumple |
| escrituras-sueltas | 7.651 | 6.666 | 1,14 | cumple |
| escrituras-concurrentes-4 | 18.679 | 6.844 | 2,72 | cumple |
| escrituras-concurrentes-16 | 22.985 | 6.555 | 3,50 | cumple |
| lecturas-al-azar | 984.727 | 849.005 | 1,15 | cumple |
| lecturas-al-azar-4 | 2.709.866 | 3.257.290 | 0,83 | cumple |
| lecturas-al-azar-16 | 3.713.092 | 3.788.998 | 0,97 | cumple |
| recorrido | 25.137.123 | 10.168.523 | 2,47 | cumple |
| intervalos | 17.788.090 | 16.354.379 | 1,08 | cumple |
| lecturas-con-escritor | 1.406.650 | 1.646.292 | 0,85 | cumple |
| escritor-con-lectores | 48.098 | 53.637 | 0,89 | cumple |
| borrado | 113.898 | 96.069 | 1,18 | cumple |
| reutilizacion | 110.715 | 98.142 | 1,12 | cumple |
| apertura | 54 | 2 | 35,32 | cumple |
| valores-grandes-carga | 6.289 | 5.469 | 1,15 | cumple |
| valores-grandes-lectura | 141.421 | 103.438 | 1,36 | cumple |
| multivalor-carga | 123.561 | 114.074 | 1,08 | cumple |
| multivalor-lectura | 7.971.608 | 7.540.369 | 1,05 | cumple |
| tamano-fijo-carga | 138.824 | 143.300 | 0,96 | cumple |
| tamano-fijo-lectura | 2.027.880 | 1.424.353 | 1,42 | cumple |
| tabla-insercion-en-orden | 438.284 | 297.075 | 1,47 | cumple |
| tabla-insercion-al-azar | 93.876 | 98.688 | 0,95 | cumple |
| tabla-carga-masiva | 445.511 | 424.541 | 1,04 | cumple |
| tabla-lecturas | 672.346 | 746.224 | 0,90 | cumple |
| tabla-intervalos | 10.966.017 | 10.736.318 | 1,02 | cumple |
| tabla-actualizacion | 109.604 | 123.588 | 0,88 | cumple |
| tabla-borrado | 117.658 | 50.554 | 2,32 | cumple |

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
| tabla-borrado | 134.217.728 | 939.524.420 |

## Opciones cambiadas

Ninguna.
