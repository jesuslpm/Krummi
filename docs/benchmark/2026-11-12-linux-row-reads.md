# Banco comparativo de Krummi

- Fecha: 2026-11-12 22:20 (UTC+00:00)
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
| carga-en-orden | 162.883 | 151.118 | 1,07 | cumple |
| carga-al-azar | 65.923 | 60.089 | 1,09 | cumple |
| carga-masiva | 372.706 | 308.837 | 1,20 | cumple |
| escrituras-sueltas | 203 | 199 | 1,01 | cumple |
| escrituras-concurrentes-4 | 413 | 197 | 2,09 | cumple |
| escrituras-concurrentes-16 | 1.691 | 200 | 8,45 | cumple |
| lecturas-al-azar | 793.927 | 759.433 | 1,04 | cumple |
| lecturas-al-azar-4 | 3.474.369 | 3.244.336 | 1,07 | cumple |
| lecturas-al-azar-16 | 5.221.078 | 4.195.771 | 1,24 | cumple |
| recorrido | 17.278.130 | 11.775.304 | 1,46 | cumple |
| intervalos | 13.318.492 | 15.246.537 | 0,87 | cumple |
| lecturas-con-escritor | 2.903.943 | 2.578.528 | 1,12 | cumple |
| escritor-con-lectores | 16.046 | 14.041 | 1,14 | cumple |
| borrado | 70.089 | 58.867 | 1,19 | cumple |
| reutilizacion | 68.122 | 58.931 | 1,15 | cumple |
| apertura | 240 | 3 | 88,29 | cumple |
| valores-grandes-carga | 4.119 | 3.089 | 1,33 | cumple |
| valores-grandes-lectura | 159.480 | 97.429 | 1,63 | cumple |
| multivalor-carga | 64.178 | 62.518 | 1,02 | cumple |
| multivalor-lectura | 8.723.293 | 6.779.912 | 1,28 | cumple |
| tamano-fijo-carga | 62.248 | 64.847 | 0,95 | cumple |
| tamano-fijo-lectura | 1.874.508 | 1.590.238 | 1,17 | cumple |
| tabla-insercion-en-orden | 144.517 | 121.010 | 1,19 | cumple |
| tabla-insercion-al-azar | 62.462 | 58.207 | 1,07 | cumple |
| tabla-carga-masiva | 510.958 | 393.583 | 1,29 | cumple |
| tabla-lecturas | 719.782 | 649.369 | 1,10 | cumple |
| tabla-intervalos | 9.564.327 | 10.046.109 | 0,95 | cumple |
| tabla-actualizacion | 72.585 | 70.058 | 1,03 | cumple |
| tabla-borrado | 72.748 | 38.252 | 1,90 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron |
|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 |
| carga-al-azar | 268.435.456 | 671.088.964 |
| carga-masiva | 268.435.456 | 402.719.044 |
| borrado | 268.435.456 | 671.088.964 |
| reutilizacion | 268.435.456 | 671.088.964 |
| valores-grandes-carga | 536.870.912 | 939.524.420 |
| multivalor-carga | 33.554.432 | 335.544.644 |
| tamano-fijo-carga | 33.554.432 | 570.425.668 |
| tabla-insercion-en-orden | 134.217.728 | 369.099.076 |
| tabla-insercion-al-azar | 268.435.456 | 671.088.964 |
| tabla-carga-masiva | 134.217.728 | 402.719.044 |
| tabla-borrado | 134.217.728 | 671.088.964 |

## Opciones cambiadas

Ninguna.
