# Write optimizations: remembered checks of written pages, journal compression, range step, recent leaves, loop alignment

Objective: the six optimizations left after the write-margin run. Branch `feat/write-optimizations` from
`feat/write-margin` (`10077aa`, unmerged; the PRD commit `10bba76` is the first of the run). Delivery: one commit per
task on the branch, no merge or push. Tests: `cargo test --workspace`; acceptance with Krummi-only A/Bs and complete
`--solo-krummi-voron` runs on Windows and WSL 2. The user asked to finish without asking: every decision is taken by
the agent and listed in the closing report.

## Specs

- **S1** — The PRD [pager-written-pages.prd.md](../../docs/prd/krummi/pager/pager-written-pages.prd.md) (Spanish,
  authoritative), decisions Q1–Q7 and criteria AC-Q1–AC-Q10, with its annotations to the ancestry. Quoted, Q1: «Las
  páginas que I4 deja de recordar antes de que el traslado las escriba (…), salvo las de un valor fuera de su hoja,
  quedan recordadas como comprobadas, con ese código, en cuanto sus bytes quedan escritos en su sitio»; Q3: «La
  versión que publica la confirmación de una hoja que cumple Q3 conserva la comprobación de organización»; Q4: «En las
  compilaciones de depuración, toda lectura por otra transacción de una hoja que conserva su comprobación por Q3 (…)
  comprueba además su organización, fuera de las comprobaciones que cuentan los criterios».
- **S2** — No compression when it does not help (W17 `valores-grandes-carga`): the journal stops spending time
  compressing data that does not get smaller, within J16 of the journal PRD («los datos que no se comprimen se guardan
  igualmente, sin error. La compresión no cambia nada de lo que leen las transacciones ni de lo que contiene
  `krummi.data`»); the algorithm is left to the implementation. No change of behavior or of what recovery accepts.
- **S3** — The step between entries of a range (W11, W27): look for a measured gain in the per-entry step of a range
  scan; keep a change only if an A/B shows it. No change of behavior.
- **S4** — A cache of recent leaves, as Voron's recently found pages, for writes and reads whose keys come close
  together (W19, W1, W23): keep only if an A/B shows a gain and nothing worse. No change of behavior or stored bytes.
- **S5** — Loop alignment in the release build (the W20 gap of the write-margin run): keep only if an A/B over all
  loads shows no load worse and W20 back. A build setting: Risk item 5.
- **S6** — Acceptance: per-change Krummi-only A/Bs (at least ten rounds for loads under one second per repetition or
  with several threads, three for the others), complete runs on Windows and WSL 2 under the bench power plan, the
  closing report with every measured figure.

Out of scope: anything the PRD's «Fuera de alcance» lists; R2 of the write-margin code review (pager writer GC).

## Tasks

| ID | Specs | Route | Status | Commit / evidence |
|---|---|---|---|---|
| T1 | S1 (Q1–Q7) | delegated writer (context backstop; parallel with T2, T3) | pending | |
| T2 | S2 | delegated writer (parallel) | pending | |
| T3 | S3 | delegated writer (parallel) | pending | |
| T4 | S4 | delegated writer after T1 (shares `krummi-tree/src/lib.rs`) | pending | |
| T5 | S5 | inline, at the end | pending | |
| T6 | S6 | inline, background runs | pending | |
| T7 | — | close: RDD of the last slice, Codex review, report, archive | pending | |

Route evidence: the parent context is past the long-session backstop (this session carries two earlier runs), and
T1, T2 and T3 are independent units with disjoint edit surfaces, each several files or 100+ lines.

## Log

- **L1** (2026-11-16) — The user's requests, verbatim: «Hagamos todas las optimizaciones»; to the estimate (8–12 h):
  «Sí, termina la ejecución sin preguntar, toma tú las decisiones». Start 2026-11-16 18:38:05 +0000. The PRD of S1
  was written and approved earlier in the session (decisions «Sí, como Voron», «Sí, solo valor igual», «Sí, se
  encadena»; Codex PRD review approved in round 2). The untracked `docs/prd/krummi/tables/tables-indexes.prd.md` is
  not part of the run and is never staged.
