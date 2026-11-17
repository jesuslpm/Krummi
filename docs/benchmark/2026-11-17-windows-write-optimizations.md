# Banco comparativo de Krummi

- Fecha: 2026-11-17 00:32 (UTC+00:00)
- Ejecución: completa, solo Krummi y Voron
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Núcleos: 16 lógicos de rendimiento de 32 (los motores se miden en ellos)
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: 8b54d0b35de1 (con cambios sin confirmar)
- Motores: Voron 7.2.7 (.NET 10.0.5)
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 29 de 29 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 963.251 | 678.115 | 1,42 | cumple |
| carga-al-azar | 118.126 | 110.102 | 1,07 | cumple |
| carga-masiva | 428.138 | 356.431 | 1,20 | cumple |
| escrituras-sueltas | 8.333 | 7.263 | 1,14 | cumple |
| escrituras-concurrentes-4 | 20.115 | 7.076 | 2,84 | cumple |
| escrituras-concurrentes-16 | 23.047 | 7.151 | 3,22 | cumple |
| lecturas-al-azar | 1.023.209 | 838.746 | 1,21 | cumple |
| lecturas-al-azar-4 | 4.309.908 | 3.549.099 | 1,21 | cumple |
| lecturas-al-azar-16 | 6.059.019 | 4.200.010 | 1,44 | cumple |
| recorrido | 31.067.189 | 10.322.772 | 3,00 | cumple |
| intervalos | 20.647.174 | 16.753.817 | 1,23 | cumple |
| lecturas-con-escritor | 2.839.096 | 1.669.843 | 1,70 | cumple |
| escritor-con-lectores | 91.279 | 52.929 | 1,72 | cumple |
| borrado | 123.110 | 103.636 | 1,18 | cumple |
| reutilizacion | 116.258 | 104.968 | 1,10 | cumple |
| apertura | 65 | 2 | 42,57 | cumple |
| valores-grandes-carga | 6.563 | 5.728 | 1,14 | cumple |
| valores-grandes-lectura | 172.283 | 109.681 | 1,57 | cumple |
| multivalor-carga | 130.106 | 121.495 | 1,07 | cumple |
| multivalor-lectura | 7.967.778 | 7.653.616 | 1,04 | cumple |
| tamano-fijo-carga | 152.415 | 152.073 | 1,00 | cumple |
| tamano-fijo-lectura | 2.134.137 | 1.805.987 | 1,18 | cumple |
| tabla-insercion-en-orden | 456.983 | 328.035 | 1,39 | cumple |
| tabla-insercion-al-azar | 97.459 | 105.431 | 0,92 | cumple |
| tabla-carga-masiva | 451.468 | 431.279 | 1,04 | cumple |
| tabla-lecturas | 754.015 | 755.819 | 0,99 | cumple |
| tabla-intervalos | 11.274.051 | 10.906.735 | 1,03 | cumple |
| tabla-actualizacion | 124.833 | 130.307 | 0,95 | cumple |
| tabla-borrado | 120.911 | 53.688 | 2,25 | cumple |

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
