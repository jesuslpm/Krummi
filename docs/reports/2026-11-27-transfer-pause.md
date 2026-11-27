# Pausa de la transferencia sin carrera: R1 de la revisión de ci-green corregido y verificado

Informe de cierre de `transfer-pause` (2026-11-27). Enfoque de ejecución: arreglo pequeño en línea (ODD, ruta
pequeña). Fuente de producto: ninguna (corrección de un fallo de pruebas sin cambio de producto). Documento de
ejecución: ninguno. Origen: R1 de la [revisión de código de ci-green](../reviews/ci-green/2026-11-27-code.md), que
quedó pendiente en el [cierre de ci-green](2026-11-27-ci-green.md).

Rama `fix/transfer-pause` con cuatro commits locales sobre `dab9d1d2`, sin fusionar ni publicar.

## Resultado

- **Especificación:** `set_transfer_paused(true)` ya no vuelve mientras el hilo de transferencia tiene una pasada
  tomada que aún no ha escrito. `next_work` marca `passing` bajo el mutex de la señal al devolver `Work::Pass`,
  `after_work` la limpia y avisa por `answered`, y `Signal::set_paused(true)` espera mientras haya una pasada tomada
  y el hilo viva. Si el hilo muere por un pánico, como en AC-E13, `Alive` ya limpia `alive` y avisa por el mismo
  condvar.
- **Pruebas:** `transfer::tests::pausing_waits_for_a_pass_already_taken` reproduce la ventana de forma determinista.
  Falló antes del arreglo ("the pause waits for the pass") y pasa después, con la sonda del verificador (`b9dd9078`).
  `cargo fmt --all --check` y `cargo clippy --workspace --all-targets -- -D warnings`: limpios.
- **Linux** (runner, `03f0b2ef`, `work/runner/20261127T174157.168Z-03f0b2efc1c5-run`): AC-E13 pasó 200 de 200
  ejecuciones y `cargo test --workspace` pasó en 255 s.
- **Windows** (`03f0b2ef`): `cargo test --workspace` falló solo
  `worker::ac_vi3_the_index_follows_the_writes` y `worker::ac_vi15_the_lists_of_large_tables`, en sus límites de
  120 s, mientras el verificador ejecutaba otro `cargo test` en la máquina. Los cuatro `worker::ac_vi*` pasaron
  después solos en 75 s. `cargo test --workspace` no se repitió completo en Windows.
- **Verificador independiente** (cambio de concurrencia, ítem 4): PASS en "ninguna pasada tomada antes de la pausa
  escribe después", "la pausa nunca se cuelga" y "producción sin cambios". Volvió a ejecutar AC-E13 y AC-E11 40 de 40
  cada uno, y todas las pruebas de `krummi-tx` y `krummi-pager`. Su sonda está en `b9dd9078`.
- **Revisiones:** RDD aprobada (abajo). El seguimiento de Codex
  ([informe](../reviews/ci-green/2026-11-27-code-02.md),
  [resoluciones](../reviews/ci-green/2026-11-27-code-resolutions.md)) aprobó: "R1 verified resolved; no open
  findings or regressions in the requested follow-up scope."

## Revisiones RDD

| Commits | Lentes | Resultado | Hallazgos y resolución | Lineage |
|---|---|---|---|---|
| `dab9d1d2..b9dd9078` | resilience, reliability | aprobada | resilience: ninguno. reliability: R3-001 (WARNING, "si algún camino de `run` saltase `after_work` tras una pasada, la pausa esperaría siempre"), descartado: cada trabajo que no es `Stop` llega a `signal.after_work` (`crates/krummi-pager/src/transfer.rs:331`) y un pánico pasa por `Alive`; R3-002 (SUGGESTION, prueba negativa de 200 ms), descartado: la prueba falló en esa aserción con el código anterior | `review-c3f511c408f7ac2d` |

La evaluación dio `medium` / `under_budget` y se escaló al ítem 4 (concurrencia), que la hizo `high_risk`.

## Tiempo

- Estimación: no se dio (arreglo pequeño, sin estimación previa). Inicio: tras el push de `main`, a las 17:37:53 UTC
  (creación de la ejecución de CI 46043350080). Fin: 17:59 UTC (`date -u`).
- **Duración activa: unos 22 min de reloj**, sin esperas al usuario registradas.

### Tiempo por fase

No medido por fase: no se llevó registro de fases.

## Informe de cambios (`dab9d1d2..0d5ecbba`)

| Área | + | − | Código + | Comentarios + | Archivos |
|---|---|---|---|---|---|
| `docs` | 117 | 0 | — | — | 2 |
| `crates/krummi-pager` | 64 | 4 | 51 | 11 | 1 |
| **Total** | **181** | **4** | **51** | **11** | **3** |

Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell,
JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos.

Sin archivos de bloqueo ni generados. Este informe no está incluido en la tabla.

## Resumen

Pausar la transferencia en una prueba ahora garantiza que ninguna pasada ya despachada escribe después. Así, un fallo
inyectado para el escritor del journal no lo consume la transferencia. El cambio en producción se limita a una marca
en el estado de la señal y a un aviso al final de cada pasada. La espera solo ocurre al pausar, que solo usan las
pruebas.

## Decisiones tomadas sin preguntar

- Reutilizar el condvar `answered`, que `Alive` ya avisa al morir el hilo, en lugar de añadir otro.
- Mantener la espera sobre `flushing` en `set_transfer_paused`, que sigue cubriendo las demandas en curso.

## Pendiente (no bloqueante)

- **Demandas tomadas que aún no tienen `flushing`** (advertencia del verificador): siguen fuera de `passing`, como
  antes del cambio. Ninguna prueba que pausa lanza demandas durante la ventana. R1 se refería solo a las pasadas.
- **`worker::ac_vi3` y `worker::ac_vi15` en Windows:** sus límites de 120 s quedan cortos cuando la máquina está
  cargada. Ya pasaba antes de este cambio.
- **Fusión:** `fix/transfer-pause` sin fusionar en `main`. Fusionar lo decide el usuario.

**Riesgo:** ítem 4 (concurrencia: el mutex y el condvar de la señal del hilo de transferencia). Lo cubren el
verificador independiente, la revisión RDD escalada y el seguimiento de Codex.
