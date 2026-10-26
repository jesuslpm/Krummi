# Codex cross review — ODD execution document

Objective: let Claude run the Claude–Codex cross review of a PRD or a finished change through a reusable skill and
a small PowerShell wrapper, with a bounded review → fix → verify loop. Tooling only, no product change (no PRD).
Branch: `feat/codex-review`. Delivery: one commit per task on the branch; no merge, no push.
Test runner: Pester 3.4 (`Invoke-Pester`) for the wrapper; a live controlled review for the whole flow.
Estimate: 1 h – 1 h 45 min. Real start: 2026-10-26 07:32:47 +0000.

## Specs

- **S1** — Wrapper: "Invocar Codex para revisar un PRD o una implementación terminada" and "Recoger y conservar el
  informe independiente". One invocation per call; the report keeps the `AGENTS.md` naming and is never
  overwritten ("Conservar `AGENTS.md`, las skills de revisión de Codex y los informes inmutables").
- **S2** — Least privilege: "Utiliza el mínimo permiso necesario para Codex. Prefiere `read-only` en revisiones de
  PRD y, si es viable sin complicar la solución, también en revisiones de código. Usa `workspace-write` cuando
  sea necesario para ejecutar pruebas, manteniendo la comprobación de cambios antes y después. No introduzcas
  infraestructura compleja de aislamiento." Also "Codex debe seguir siendo exclusivamente revisor, sin modificar
  el producto" and "condex debe poder ejecutar cargo test y escribir los informes de revisión".
- **S3** — "Un error de ejecución de Codex no equivale a una aprobación."
- **S4** — Independence and scope: "Preservar la independencia de la primera revisión." and "Las revisiones de
  seguimiento deben limitarse a los hallazgos pendientes y las regresiones de sus correcciones."
- **S5** — Traceability: "Mantener la trazabilidad de cada revisión, corrección y verificación."
- **S6** — Loop (skill): analyze findings, "corregir los defectos demostrados", "Registrar las resoluciones
  siguiendo las convenciones existentes", "Invocar nuevamente Codex para verificar las correcciones", "Repetir si
  aparecen nuevos hallazgos, con un máximo configurable de iteraciones" ("Mantén el límite de tres rondas").
- **S7** — Disagreement: "No corrijas automáticamente los hallazgos técnicos que consideres incorrectos.
  Regístralos como `dismissed with evidence`, aportando una justificación comprobable, y deja que Codex verifique
  esa resolución. Si persiste una discrepancia técnica relevante, escálamela junto con la evidencia de ambos."
- **S8** — "No modificar requisitos de producto sin consultarme." Product boundary accepted by the user: in a PRD
  review only consistency, evidence and wording are fixed; anything changing behavior, messages or scope, or an
  open question, stays `pending` and is asked at the end. "No fusionar ramas automáticamente." No push.
- **S9** — "Evitar duplicar la infraestructura de orquestación ya existente."
- **S10** — "Actualiza primero la CLI de Codex" and "valida el flujo completo mediante una prueba controlada, sin
  modificar código de producto ni crear commits innecesarios."

## Defaults taken

- Every review runs in `workspace-write`, with the before/after check as the only guard (L5). The wrapper fixes the
  report path (next free `docs/reviews/<feature>/<date>-<kind>[-NN].md`, local date) and allows exactly that new
  file; build artifacts and test temporaries are ignored by git and so outside the check.
- Follow-ups are fresh `codex exec` sessions (no resume); the report and the resolutions carry the context. The
  wrapper takes the chain's original report; a code follow-up also takes the range of the fix commits.
- One resolutions file per review chain: the original report's `-resolutions.md` (as in `docs/reviews/trees-keys`).
- Preflight refuses to run when `codex doctor` shows the project model is not loaded (untrusted path, for
  example a worktree copy), because Codex then silently uses the user's model and no project agents.
- Telemetry (real model/effort per thread, from `~/.codex/sessions`) is best effort: when it fails the result says
  why and the status is unaffected.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T0 | S10 | inline | done | npm `@openai/codex` 0.138.0 → 0.164.0 |
| T1 | S1–S5 | inline | done | see Log L6 |
| T2 | S6–S9 | inline | done | see Log L7 |
| T3 | S10 | inline | in progress | |

## Log

- **L1** (2026-10-26, user, verbatim): "Quiero automatizar el proceso de revisión cruzada Claude–Codex que ya
  estamos utilizando en Krummi. Estudia cómo integrar la CLI de Codex en nuestro flujo actual de PRD y ODD,
  preferiblemente mediante una skill reutilizable y un pequeño script de PowerShell. […] Prioriza simplicidad,
  robustez y mantenimiento." (full request in the session; requirements quoted in the Specs).
- **L2** (2026-10-26, user): "Lo que usé fue la extensión de VS de codex. Seguramente debería actualizar la cli.",
  then "Espera, condex debe poder ejecutar cargo test y escribir los informes de revisión.", then the approval
  with two refinements (S2, S7): "Sí, me parece bien la propuesta y acepto la frontera de producto que planteas."
- **L3** (2026-10-26, evidence before start): npm codex 0.138.0 failed with "The 'gpt-6.1-sol' model is not
  supported when using Codex with a ChatGPT account" (exit 1, `turn.failed`); the VS Code extension bundles
  0.163.0-alpha. Codex 0.163 `exec` in the repo loads `gpt-6.1-sol`/`high`, sees `review-prd`, `review-code`,
  `krummi-review`, `krummi-review-critical`; exec approval policy is `never`. Rollouts in `~/.codex/sessions`
  record the real model/effort per thread (`turn_context`) and subagents carry `parent_thread_id` and
  `agent_role`: past reviews ran `gpt-6-astra`/`xhigh` and `gpt-6.1-sol`/`high` in `workspace-write`, and
  `cargo test --locked --workspace` passed there.
- **L4** (2026-10-26 07:33): CLI updated to 0.164.0. `codex doctor` shows `gpt-6.1-sol` in the repo but `gpt-5.5`
  in an untrusted clone, even with a `-c projects.'<path>'.trust_level` override.
- **L5** (2026-10-26 07:38, user, verbatim): "Pero asegúrate de que codex puede escribir los informes de revisión y
  ejecutar los tests." Probe the same minute: `codex exec -s read-only --add-dir <dir>` denied writes both in the
  added folder and in `target/` ("Access to the path … is denied"), so `read-only` cannot coexist with Codex saving
  its report. Decision: `workspace-write` for every review (must over the "prefer `read-only`" preference of S2),
  guarded by the before/after check; the earlier rule choosing the sandbox by the files touched is dropped.
- **L6** (2026-10-26 07:46): T1 wrapper, output schema and Pester tests with a fake Codex (`tests/fake-codex.*`):
  15/15 green (approved, `-02` naming, code range, findings, failed run, invalid result, missing report, timeout,
  tracked change and new commit as violations, ignored artifacts and pre-existing changes allowed, untrusted
  config, old CLI, code follow-up prompt, follow-up refusals). The tests were written after the script, not
  RED-first; the first run failed 8 cases, all test-harness defects (fake `CODEX_HOME` inside the test repo, which
  the guard flagged; console encoding in the fake; fake rollout lines joined), plus one wrapper bug: settings read
  from a dictionary through `Get-Prop` lost the reviewers' model. Silent telemetry failures now surface in the result.
- **L7** (2026-10-26 07:48): T2 skill `.claude/skills/codex-review/SKILL.md` (run table, status table, loop with
  fix / dismiss with evidence / pending, stop conditions, traceability) and one line in `CLAUDE.md` *External review
  reports*. No change to `AGENTS.md`, `.codex/` or `.agents/skills/`: the wrapper's prompt is the plain request plus
  a mechanical contract (report path, output schema), which `AGENTS.md` already accommodates.
- **L8** (2026-10-26 07:48): first live run (T3) stopped at the preflight with status `error` ("Codex does not load
  the project config here (model '' …)"), so no review ran and nothing was approved. Cause: in the real
  `codex doctor --json` the key is literally `"config.load"`; the wrapper and the fake read `config` → `load`. Both
  now follow the real output; 15/15 green.
- **L9** (2026-10-26, Codex round 1): run 07:49:44–07:56:39 on `c6fa485..f46e9d7`, status `findings`, report
  [2026-10-26-code.md](../../docs/reviews/codex-review/2026-10-26-code.md), thread
  `01a1772f-da2c-7f32-85a4-c1e0777075bc`, executed `gpt-6.1-sol`/`high` (coordinator and one `krummi-review`).
  The guard saw only the new report. R1 major (follow-up refused a numbered original report) → fixed `c47b0be`;
  R2 major (`taskkill` denied in Codex's sandbox left the post-timeout wait unbounded) → fixed `7afccd2`; the
  report's unproven observation on a string `approved` → hardened `e670fb4`. Pester 17/17. Resolutions:
  [2026-10-26-code-resolutions.md](../../docs/reviews/codex-review/2026-10-26-code-resolutions.md).
- **L10** (2026-10-26, Codex round 2): follow-up run 08:00:59–08:04:24 on fixes `f46e9d7..ae51675`, status
  `findings`, report [2026-10-26-code-02.md](../../docs/reviews/codex-review/2026-10-26-code-02.md), thread
  `01a1773a-2e99-7431-a089-058a89b4aca3`, executed `gpt-6.1-sol`/`high` (coordinator and one `krummi-review`).
  R2 verified resolved; R1 still open as minor (three-digit originals such as `-100`) → fixed `2499228`, Pester 18/18.
