# Banco comparativo de Krummi

- Fecha: 2026-11-11 07:54 (UTC+00:00)
- Ejecución: completa
- Sistema: Windows 11 Pro 10.0.26200
- Procesador: 13th Gen Intel(R) Core(TM) i9-13980HX, 32 núcleos lógicos
- Memoria: 64 GiB
- Disco: NVMe SAMSUNG MZVL22T0HBLB-00B00, NTFS
- Krummi: d0d4ef66bb64 (con cambios sin confirmar)
- Motores: Voron 7.2.7 (.NET 10.0.5), LMDB 0.9.70 (heed 0.23.1), redb 4.4.0, canopydb 0.2.5, fjall 3.1.13
- Rust: 1.99.0
- Repeticiones: 1 previa y 3 medidas, mediana

## Resultado

Krummi cumple 26 de 29 umbrales. Incumple: `tabla-lecturas`, `tabla-intervalos`, `tabla-actualizacion`.

## Operaciones por segundo

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall | Krummi / Voron | Umbral |
|---|---|---|---|---|---|---|---|---|
| carga-en-orden | 943.198 | 666.145 | 175.838 | 249.205 | 328.389 | 248.080 | 1,41 | cumple |
| carga-al-azar | 115.218 | 109.328 | 32.357 | 18.838 | 174.857 | 198.121 | 1,05 | cumple |
| carga-masiva | 416.967 | 346.972 | 97.771 | 336.674 | 181.081 | 258.790 | 1,20 | cumple |
| escrituras-sueltas | 8.132 | 7.309 | 3.770 | 390 | 434 | 457 | 1,11 | cumple |
| escrituras-concurrentes-4 | 17.293 | 7.260 | 3.591 | 389 | 446 | 455 | 2,38 | cumple |
| escrituras-concurrentes-16 | 22.898 | 7.158 | 3.550 | 385 | 442 | 439 | 3,19 | cumple |
| lecturas-al-azar | 1.001.640 | 878.408 | 1.020.494 | 842.512 | 831.774 | 274.606 | 1,14 | cumple |
| lecturas-al-azar-4 | 4.210.822 | 3.469.650 | 4.238.548 | 2.463.326 | 2.496.389 | 933.927 | 1,21 | cumple |
| lecturas-al-azar-16 | 5.040.677 | 3.218.968 | 6.401.570 | 2.750.023 | 1.998.524 | 860.017 | 1,56 | cumple |
| recorrido | 18.479.575 | 8.822.878 | 14.777.903 | 11.792.105 | 7.013.828 | 2.411.195 | 2,09 | cumple |
| intervalos | 14.103.067 | 15.414.657 | 13.514.548 | 9.566.136 | 6.293.583 | 1.926.274 | 0,91 | cumple |
| lecturas-con-escritor | 2.417.565 | 2.146.732 | 1.959.014 | 1.909.186 | 1.512.779 | 779.400 | 1,12 | cumple |
| escritor-con-lectores | 83.094 | 65.720 | 8.898 | 6.233 | 27.225 | 36.133 | 1,26 | cumple |
| borrado | 118.009 | 100.943 | 29.111 | 8.995 | 43.174 | 212.123 | 1,16 | cumple |
| reutilizacion | 111.585 | 102.824 | 28.244 | 6.365 | 44.837 | 208.201 | 1,08 | cumple |
| apertura | 64 | 2 | 2.020 | 64 | 109 | 1 | 41,91 | cumple |
| valores-grandes-carga | 5.193 | 5.578 | 3.481 | 2.054 | 3.685 | 7.157 | 0,93 | cumple |
| valores-grandes-lectura | 149.090 | 105.994 | 162.781 | 154.950 | 141.125 | 129.736 | 1,40 | cumple |
| multivalor-carga | 117.792 | 118.395 | 40.871 | 13.781 | no aplica | no aplica | 0,99 | cumple |
| multivalor-lectura | 8.590.150 | 7.155.978 | 9.611.651 | 6.530.800 | no aplica | no aplica | 1,20 | cumple |
| tamano-fijo-carga | 148.104 | 150.135 | 43.847 | 14.873 | no aplica | no aplica | 0,98 | cumple |
| tamano-fijo-lectura | 1.966.274 | 1.752.680 | 1.226.332 | 1.439.986 | no aplica | no aplica | 1,12 | cumple |
| tabla-insercion-en-orden | 372.211 | 316.685 | no aplica | no aplica | no aplica | no aplica | 1,17 | cumple |
| tabla-insercion-al-azar | 95.019 | 103.349 | no aplica | no aplica | no aplica | no aplica | 0,91 | cumple |
| tabla-carga-masiva | 372.402 | 419.675 | no aplica | no aplica | no aplica | no aplica | 0,88 | cumple |
| tabla-lecturas | 557.063 | 739.806 | no aplica | no aplica | no aplica | no aplica | 0,75 | incumple |
| tabla-intervalos | 1.696.384 | 10.657.201 | no aplica | no aplica | no aplica | no aplica | 0,15 | incumple |
| tabla-actualizacion | 94.100 | 127.190 | no aplica | no aplica | no aplica | no aplica | 0,73 | incumple |
| tabla-borrado | 115.524 | 52.412 | no aplica | no aplica | no aplica | no aplica | 2,20 | cumple |

## Tamaño del almacén (bytes)

| Carga | Krummi | Voron | LMDB | redb | canopydb | fjall |
|---|---|---|---|---|---|---|
| carga-en-orden | 134.217.728 | 234.881.348 | 133.054.400 | 134.746.112 | 208.160.787 | 160.711.632 |
| carga-al-azar | 268.435.456 | 671.088.964 | 200.875.968 | 269.488.128 | 624.054.571 | 160.945.569 |
| carga-masiva | 268.435.456 | 402.719.044 | 189.239.232 | 269.488.128 | 344.170.216 | 191.723.778 |
| borrado | 268.435.456 | 671.088.964 | 201.248.704 | 585.728 | 561.860.907 | 203.692.607 |
| reutilizacion | 268.435.456 | 671.088.964 | 208.777.152 | 268.701.696 | 591.070.262 | 238.204.041 |
| valores-grandes-carga | 536.870.912 | 671.088.964 | 285.802.432 | 538.972.160 | 625.836.331 | 336.042.541 |
| multivalor-carga | 33.554.432 | 536.871.236 | 54.071.232 | 67.375.104 | no aplica | no aplica |
| tamano-fijo-carga | 33.554.432 | 570.425.668 | 46.190.528 | 44.044.288 | no aplica | no aplica |
| tabla-insercion-en-orden | 134.217.728 | 369.099.076 | no aplica | no aplica | no aplica | no aplica |
| tabla-insercion-al-azar | 268.435.456 | 671.088.964 | no aplica | no aplica | no aplica | no aplica |
| tabla-carga-masiva | 134.217.728 | 402.719.044 | no aplica | no aplica | no aplica | no aplica |
| tabla-borrado | 134.217.728 | 805.306.692 | no aplica | no aplica | no aplica | no aplica |

## Opciones cambiadas

- LMDB: tamaño del mapa de 1 GiB; claves duplicadas ordenadas en W19 y W20; claves enteras en W21 y W22.
- canopydb: confirmación que espera al disco del diario.
- fjall: caché de bloques de 1 GiB; llevar el diario al disco (SyncData) en cada confirmación.
