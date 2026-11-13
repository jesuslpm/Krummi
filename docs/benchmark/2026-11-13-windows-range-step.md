# Banco comparativo de Krummi

- Fecha: 2026-11-13 08:23 (UTC+00:00)
- Ejecución: completa, solo Krummi y Voron
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: 44b10940c4f3 (con cambios sin confirmar)
- Motores: Voron 7.2.7 (.NET 10.0.5)
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 29 de 29 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 952.556 | 645.523 | 1,47 | cumple |
| carga-al-azar | 113.646 | 109.152 | 1,04 | cumple |
| carga-masiva | 418.059 | 373.308 | 1,11 | cumple |
| escrituras-sueltas | 8.065 | 7.362 | 1,09 | cumple |
| escrituras-concurrentes-4 | 18.928 | 7.254 | 2,60 | cumple |
| escrituras-concurrentes-16 | 21.907 | 7.277 | 3,01 | cumple |
| lecturas-al-azar | 944.794 | 863.484 | 1,09 | cumple |
| lecturas-al-azar-4 | 4.202.247 | 3.521.423 | 1,19 | cumple |
| lecturas-al-azar-16 | 5.118.653 | 4.009.568 | 1,27 | cumple |
| recorrido | 22.933.518 | 10.321.387 | 2,22 | cumple |
| intervalos | 16.400.071 | 16.725.114 | 0,98 | cumple |
| lecturas-con-escritor | 2.611.417 | 2.265.484 | 1,15 | cumple |
| escritor-con-lectores | 88.843 | 72.469 | 1,22 | cumple |
| borrado | 120.076 | 101.537 | 1,18 | cumple |
| reutilizacion | 111.031 | 104.341 | 1,06 | cumple |
| apertura | 56 | 1 | 37,78 | cumple |
| valores-grandes-carga | 5.187 | 5.440 | 0,95 | cumple |
| valores-grandes-lectura | 159.998 | 112.044 | 1,42 | cumple |
| multivalor-carga | 114.110 | 117.564 | 0,97 | cumple |
| multivalor-lectura | 8.005.424 | 7.416.197 | 1,07 | cumple |
| tamano-fijo-carga | 151.960 | 149.424 | 1,01 | cumple |
| tamano-fijo-lectura | 2.120.530 | 1.801.933 | 1,17 | cumple |
| tabla-insercion-en-orden | 466.825 | 317.190 | 1,47 | cumple |
| tabla-insercion-al-azar | 96.099 | 104.095 | 0,92 | cumple |
| tabla-carga-masiva | 462.265 | 450.110 | 1,02 | cumple |
| tabla-lecturas | 737.983 | 755.287 | 0,97 | cumple |
| tabla-intervalos | 9.763.295 | 10.617.196 | 0,91 | cumple |
| tabla-actualizacion | 113.331 | 129.183 | 0,87 | cumple |
| tabla-borrado | 116.331 | 52.534 | 2,21 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron |
|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 |
| carga-al-azar | 268.435.456 | 671.088.964 |
| carga-masiva | 268.435.456 | 402.719.044 |
| borrado | 268.435.456 | 671.088.964 |
| reutilizacion | 268.435.456 | 671.088.964 |
| valores-grandes-carga | 536.870.912 | 1.006.633.284 |
| multivalor-carga | 33.554.432 | 402.653.508 |
| tamano-fijo-carga | 33.554.432 | 570.425.668 |
| tabla-insercion-en-orden | 134.217.728 | 369.099.076 |
| tabla-insercion-al-azar | 268.435.456 | 671.088.964 |
| tabla-carga-masiva | 134.217.728 | 402.719.044 |
| tabla-borrado | 134.217.728 | 805.306.692 |

## Opciones cambiadas

Ninguna.
