# Límites de espera de las pruebas del trabajador vectorial

Informe de cierre de `vector-test-limits` (2026-11-28). Se ejecutó como arreglo pequeño en línea (ODD, ruta pequeña).
No hay fuente de producto, porque es un ajuste de pruebas sin cambio de producto, ni documento de ejecución. Origen: el
pendiente de `worker::ac_vi3` y `worker::ac_vi15` del [cierre de transfer-pause](2026-11-27-transfer-pause.md).

Rama `fix/vector-test-limits`, sobre `370f7914`, sin fusionar ni publicar.

## Resultado

- **Cambio:** `ac_vi3_the_index_follows_the_writes` y `ac_vi15_the_lists_of_large_tables` esperan ahora hasta
  `LOADED_LIMIT` (600 s) en lugar de 120 s. Es una constante de `crates/krummi/tests/table_vector_indexes/worker.rs`
  documentada con el motivo. Las aserciones sobre el estado final no cambian. Las otras esperas de 120 s del módulo
  (`ready`, AC-VI17) quedan igual, porque no fallaron.
- **Comprobaciones** (Windows, `79b2b59b`):
  - `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings`: limpios.
  - `worker::`: 6 pasaron y 1 ignorada, en 39 s.
  - `cargo test --workspace`: pasó dos veces seguidas, sin fallos.
- **Revisiones:**
  - RDD aprobada (abajo).
  - Codex ([informe](../reviews/vector-test-limits/2026-11-28-code.md)): "Approved — no findings introduced in the
    requested range". Hilo `01a2216d-2d6b-7e60-b2c0-494e3eef1893`, coordinador y revisor `gpt-6.1-sol` / `high`
    según la telemetría.

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Lineage |
|---|---|---|---|---|
| `370f7914..79b2b59b` | reliability | aprobada | R3-loaded-limit-hang-latency (SUGGESTION, "a real regression ... now takes five times longer to fail"; propone una variable de entorno o el límite grande solo en debug). Descartada: las pruebas corren en debug también en CI, así que el límite solo en debug no cambiaría nada, y una variable de entorno sería una abstracción que nada pide. Las aserciones de comportamiento no cambian. | `review-e5416ae6359dc7c3` |

La evaluación dio `medium` / `under_budget`. La revisión del último tramo se pidió como indica `CLAUDE.md`.

## Tiempo

- Estimación: no se dio, porque es un arreglo pequeño. Inicio: 08:40:58 UTC (`date -u`, al crear la rama). Fin:
  09:16:29 UTC.
- **Duración activa: unos 36 min de reloj**, sin esperas al usuario. La mayor parte se fue en las dos vueltas de
  `cargo test --workspace` y en la revisión de Codex.

### Tiempo por fase

No se midió por fase: no se llevó registro de fases.

## Informe de cambios (`370f7914..79b2b59b`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `crates/krummi` | 7 | 2 | 3 | 3 | 1 |
| **Total** | **7** | **2** | **3** | **3** | **1** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell,
JavaScript/TypeScript, TOML, YAML y shell). Las líneas en blanco no cuentan en ninguna de las dos.

Sin archivos de bloqueo ni generados. La tabla no incluye este informe ni el de Codex.

## Pendiente (no bloqueante)

- **Fallo comprobado bajo carga:** no se repitió la situación que hizo fallar las pruebas, otro `cargo test` en
  paralelo en la máquina. Los 600 s dan cinco veces el margen anterior.
- **Fusión y publicación:** las decide el usuario.

**Riesgo:** ninguno. Solo cambian los límites de dos esperas de prueba. Una prueba que se colgase tardaría más en
fallar, pero seguiría fallando.
