# Banco comparativo de Krummi

- Fecha: 2026-11-17 09:42 (UTC+00:00)
- Ejecución: completa, solo Krummi y Voron
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: 886db96889a5 (con cambios sin confirmar)
- Motores: Voron 7.2.7 (.NET 10.0.5)
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 27 de 29 umbrales. Incumple: `lecturas-con-escritor`, `escritor-con-lectores`.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 929.085 | 686.100 | 1,35 | cumple |
| carga-al-azar | 118.397 | 109.211 | 1,08 | cumple |
| carga-masiva | 423.400 | 361.687 | 1,17 | cumple |
| escrituras-sueltas | 8.512 | 7.273 | 1,17 | cumple |
| escrituras-concurrentes-4 | 19.965 | 7.299 | 2,73 | cumple |
| escrituras-concurrentes-16 | 22.926 | 7.007 | 3,27 | cumple |
| lecturas-al-azar | 1.014.111 | 861.892 | 1,17 | cumple |
| lecturas-al-azar-4 | 4.331.024 | 3.570.426 | 1,21 | cumple |
| lecturas-al-azar-16 | 5.012.826 | 4.070.031 | 1,23 | cumple |
| recorrido | 30.996.795 | 11.220.499 | 2,76 | cumple |
| intervalos | 21.188.463 | 16.803.607 | 1,26 | cumple |
| lecturas-con-escritor | 1.709.208 | 2.317.680 | 0,73 | incumple |
| escritor-con-lectores | 58.558 | 73.563 | 0,79 | incumple |
| borrado | 122.884 | 103.879 | 1,18 | cumple |
| reutilizacion | 115.317 | 105.175 | 1,09 | cumple |
| apertura | 66 | 2 | 43,94 | cumple |
| valores-grandes-carga | 5.617 | 5.623 | 0,99 | cumple |
| valores-grandes-lectura | 169.253 | 117.176 | 1,44 | cumple |
| multivalor-carga | 130.313 | 114.422 | 1,13 | cumple |
| multivalor-lectura | 8.298.803 | 6.897.679 | 1,20 | cumple |
| tamano-fijo-carga | 153.735 | 150.277 | 1,02 | cumple |
| tamano-fijo-lectura | 2.131.702 | 1.725.858 | 1,23 | cumple |
| tabla-insercion-en-orden | 461.809 | 313.745 | 1,47 | cumple |
| tabla-insercion-al-azar | 99.409 | 101.520 | 0,97 | cumple |
| tabla-carga-masiva | 453.089 | 437.186 | 1,03 | cumple |
| tabla-lecturas | 749.490 | 731.500 | 1,02 | cumple |
| tabla-intervalos | 11.648.173 | 10.891.642 | 1,06 | cumple |
| tabla-actualizacion | 125.748 | 129.088 | 0,97 | cumple |
| tabla-borrado | 122.085 | 52.659 | 2,31 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron |
|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 |
| carga-al-azar | 268.435.456 | 671.088.964 |
| carga-masiva | 268.435.456 | 402.719.044 |
| borrado | 268.435.456 | 671.088.964 |
| reutilizacion | 268.435.456 | 671.088.964 |
| valores-grandes-carga | 536.870.912 | 939.524.420 |
| multivalor-carga | 33.554.432 | 536.871.236 |
| tamano-fijo-carga | 33.554.432 | 570.425.668 |
| tabla-insercion-en-orden | 134.217.728 | 369.099.076 |
| tabla-insercion-al-azar | 268.435.456 | 671.088.964 |
| tabla-carga-masiva | 134.217.728 | 402.719.044 |
| tabla-borrado | 134.217.728 | 805.306.692 |

## Opciones cambiadas

Ninguna.
