# Banco comparativo de Krummi

- Fecha: 2026-11-17 10:52 (UTC+00:00)
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

Krummi cumple 29 de 29 umbrales.

## Operaciones por segundo

| Carga | Krummi | Voron | Krummi / Voron | Umbral |
|---|---|---|---|---|
| carga-en-orden | 943.255 | 644.577 | 1,46 | cumple |
| carga-al-azar | 116.744 | 105.062 | 1,11 | cumple |
| carga-masiva | 421.610 | 338.311 | 1,24 | cumple |
| escrituras-sueltas | 8.888 | 6.205 | 1,43 | cumple |
| escrituras-concurrentes-4 | 21.196 | 6.066 | 3,49 | cumple |
| escrituras-concurrentes-16 | 23.734 | 5.924 | 4,00 | cumple |
| lecturas-al-azar | 1.062.914 | 816.646 | 1,30 | cumple |
| lecturas-al-azar-4 | 4.201.194 | 3.529.583 | 1,19 | cumple |
| lecturas-al-azar-16 | 3.299.300 | 2.865.763 | 1,15 | cumple |
| recorrido | 31.425.492 | 9.692.731 | 3,24 | cumple |
| intervalos | 21.351.950 | 16.524.354 | 1,29 | cumple |
| lecturas-con-escritor | 2.244.774 | 2.304.706 | 0,97 | cumple |
| escritor-con-lectores | 70.639 | 73.428 | 0,96 | cumple |
| borrado | 123.146 | 100.737 | 1,22 | cumple |
| reutilizacion | 115.497 | 102.643 | 1,12 | cumple |
| apertura | 59 | 2 | 38,90 | cumple |
| valores-grandes-carga | 4.696 | 5.584 | 0,84 | cumple |
| valores-grandes-lectura | 169.250 | 125.982 | 1,34 | cumple |
| multivalor-carga | 125.930 | 118.233 | 1,06 | cumple |
| multivalor-lectura | 8.467.178 | 7.742.869 | 1,09 | cumple |
| tamano-fijo-carga | 146.517 | 152.506 | 0,96 | cumple |
| tamano-fijo-lectura | 2.178.163 | 1.874.960 | 1,16 | cumple |
| tabla-insercion-en-orden | 455.362 | 307.849 | 1,47 | cumple |
| tabla-insercion-al-azar | 96.082 | 104.249 | 0,92 | cumple |
| tabla-carga-masiva | 441.645 | 441.230 | 1,00 | cumple |
| tabla-lecturas | 759.594 | 734.625 | 1,03 | cumple |
| tabla-intervalos | 11.553.933 | 10.746.030 | 1,07 | cumple |
| tabla-actualizacion | 126.285 | 128.143 | 0,98 | cumple |
| tabla-borrado | 121.760 | 52.481 | 2,32 | cumple |

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
