# Banco comparativo de Krummi

- Fecha: 2026-11-12 18:03 (UTC+00:00)
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
| carga-en-orden | 162.319 | 144.605 | 1,12 | cumple |
| carga-al-azar | 65.171 | 60.049 | 1,08 | cumple |
| carga-masiva | 377.916 | 304.516 | 1,24 | cumple |
| escrituras-sueltas | 201 | 197 | 1,01 | cumple |
| escrituras-concurrentes-4 | 417 | 190 | 2,19 | cumple |
| escrituras-concurrentes-16 | 1.743 | 181 | 9,60 | cumple |
| lecturas-al-azar | 767.783 | 753.082 | 1,01 | cumple |
| lecturas-al-azar-4 | 3.503.342 | 3.047.448 | 1,14 | cumple |
| lecturas-al-azar-16 | 4.171.214 | 3.707.691 | 1,12 | cumple |
| recorrido | 16.399.915 | 11.827.482 | 1,38 | cumple |
| intervalos | 13.498.228 | 14.845.079 | 0,90 | cumple |
| lecturas-con-escritor | 2.971.369 | 2.360.332 | 1,25 | cumple |
| escritor-con-lectores | 14.324 | 13.643 | 1,04 | cumple |
| borrado | 69.250 | 55.845 | 1,24 | cumple |
| reutilizacion | 66.443 | 57.449 | 1,15 | cumple |
| apertura | 467 | 3 | 170,80 | cumple |
| valores-grandes-carga | 3.522 | 3.239 | 1,08 | cumple |
| valores-grandes-lectura | 148.918 | 82.467 | 1,80 | cumple |
| multivalor-carga | 63.742 | 64.977 | 0,98 | cumple |
| multivalor-lectura | 8.688.950 | 6.504.932 | 1,33 | cumple |
| tamano-fijo-carga | 60.220 | 69.231 | 0,86 | cumple |
| tamano-fijo-lectura | 1.889.571 | 1.419.690 | 1,33 | cumple |
| tabla-insercion-en-orden | 130.616 | 117.762 | 1,10 | cumple |
| tabla-insercion-al-azar | 60.514 | 57.843 | 1,04 | cumple |
| tabla-carga-masiva | 411.861 | 371.015 | 1,11 | cumple |
| tabla-lecturas | 718.273 | 622.821 | 1,15 | cumple |
| tabla-intervalos | 9.498.422 | 9.917.552 | 0,95 | cumple |
| tabla-actualizacion | 70.633 | 71.146 | 0,99 | cumple |
| tabla-borrado | 71.026 | 37.908 | 1,87 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron |
|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 |
| carga-al-azar | 268.435.456 | 671.088.964 |
| carga-masiva | 268.435.456 | 402.719.044 |
| borrado | 268.435.456 | 671.088.964 |
| reutilizacion | 268.435.456 | 671.088.964 |
| valores-grandes-carga | 536.870.912 | 671.088.964 |
| multivalor-carga | 33.554.432 | 335.544.644 |
| tamano-fijo-carga | 33.554.432 | 570.425.668 |
| tabla-insercion-en-orden | 134.217.728 | 369.099.076 |
| tabla-insercion-al-azar | 268.435.456 | 671.088.964 |
| tabla-carga-masiva | 134.217.728 | 402.719.044 |
| tabla-borrado | 134.217.728 | 671.088.964 |

## Opciones cambiadas

Ninguna.
