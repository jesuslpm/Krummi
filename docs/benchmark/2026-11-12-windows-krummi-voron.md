# Banco comparativo de Krummi

- Fecha: 2026-11-12 19:10 (UTC+00:00)
- Ejecución: completa, solo Krummi y Voron
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: 47f3e8da99d5 (con cambios sin confirmar)
- Motores: Voron 7.2.7 (.NET 10.0.5)
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 29 de 29 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 933.617 | 644.476 | 1,44 | cumple |
| carga-al-azar | 110.866 | 105.178 | 1,05 | cumple |
| carga-masiva | 403.048 | 348.463 | 1,15 | cumple |
| escrituras-sueltas | 7.869 | 6.857 | 1,14 | cumple |
| escrituras-concurrentes-4 | 18.614 | 6.817 | 2,73 | cumple |
| escrituras-concurrentes-16 | 22.336 | 6.799 | 3,28 | cumple |
| lecturas-al-azar | 984.106 | 859.747 | 1,14 | cumple |
| lecturas-al-azar-4 | 4.199.994 | 3.437.153 | 1,22 | cumple |
| lecturas-al-azar-16 | 5.283.907 | 3.895.568 | 1,35 | cumple |
| recorrido | 17.865.948 | 10.077.811 | 1,77 | cumple |
| intervalos | 14.004.315 | 16.287.960 | 0,85 | cumple |
| lecturas-con-escritor | 2.006.170 | 2.145.949 | 0,93 | cumple |
| escritor-con-lectores | 73.038 | 69.138 | 1,05 | cumple |
| borrado | 115.862 | 98.258 | 1,17 | cumple |
| reutilizacion | 109.122 | 98.388 | 1,10 | cumple |
| apertura | 59 | 2 | 39,14 | cumple |
| valores-grandes-carga | 5.139 | 6.096 | 0,84 | cumple |
| valores-grandes-lectura | 168.264 | 110.228 | 1,52 | cumple |
| multivalor-carga | 113.503 | 115.233 | 0,98 | cumple |
| multivalor-lectura | 7.979.993 | 7.268.504 | 1,09 | cumple |
| tamano-fijo-carga | 144.299 | 143.814 | 1,00 | cumple |
| tamano-fijo-lectura | 2.017.533 | 1.687.133 | 1,19 | cumple |
| tabla-insercion-en-orden | 364.056 | 308.003 | 1,18 | cumple |
| tabla-insercion-al-azar | 91.460 | 99.561 | 0,91 | cumple |
| tabla-carga-masiva | 361.850 | 418.483 | 0,86 | cumple |
| tabla-lecturas | 706.884 | 733.829 | 0,96 | cumple |
| tabla-intervalos | 9.172.452 | 11.140.170 | 0,82 | cumple |
| tabla-actualizacion | 109.913 | 127.159 | 0,86 | cumple |
| tabla-borrado | 115.681 | 53.072 | 2,17 | cumple |

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
