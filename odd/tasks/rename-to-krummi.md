# Rename to Krummi — ODD execution document

Objective: implement [rename-to-krummi.prd.md](../../docs/prd/muninndb/rename-to-krummi.prd.md) (R1–R13, AC-R1–AC-R13):
the product MuninnDB becomes Krummi in packages, store files, mark, format, messages, docs, PRDs, tooling, GitHub
repository and memory project.
Branch: `feat/rename-to-krummi`, in the main copy (R9). Delivery: `ask-on-risk`. Test runner: `cargo test --workspace`
(Windows; Linux in WSL). Estimate: 1 h 15 min – 2 h. Real start: 2026-10-23 08:39:40 +0100.

## Specs

The PRD is the specification; every S# below points at its decisions and criteria, which workers read in full.

- **S1 — Packages (R2, AC-R5).** Packages `krummi`, `krummi-pager`, `krummi-tx`; `muninndb*` no longer exist; public
  type and function names unchanged; no path contains `muninn` (R10).
- **S2 — Store files, mark and format (R3, R4, R5, AC-R1, AC-R3, AC9 adapted).** `krummi.data`, `krummi.data.new`,
  `krummi.journal.` + 19 digits, `krummi.temp`; a new mark; format `3`; "AC9 se adapta a R5: formato anterior con `0`
  y `2`, posterior con `4`."
- **S3 — MuninnDB leftovers (R6, AC-R2, AC-R4).** A folder without `krummi.data` and with any `muninn.*` entry is not
  a store and is left untouched; inside a Krummi store, `muninn.*` entries are foreign and survive use, close and
  recovery byte for byte.
- **S4 — Error texts (R1, AC-R6).** The library errors that name the product today (not a store, different format)
  contain `Krummi` and no `muninn` in any case.
- **S5 — Docs (R7, R12, AC-R8).** README explains the name Krummi ("Krummi, el apodo islandés del cuervo, y el guiño a
  Voron") without naming the old one; instructions, worktree guide and methodology name `C:\projects\Krummi` and
  copies `C:\wt\krummi-<n>`.
- **S6 — Working copies (R8, AC-R9).** Copies `krummi-<n>` (folder and memory project); their tests pass on Windows.
- **S7 — PRDs (R7, R11, AC-R7, AC-R11).** Folder `docs/prd/krummi`, root `krummi.prd.md` / `krummi.prd.en.md`;
  titles, rules, flows, messages, outputs and criteria renamed; an annotation per PRD; history sections keep their
  text; every relative link and anchor works.
- **S8 — Remaining name (R10, AC-R7).** `git grep -i muninn` only in R10 (a)–(e); no tracked path contains `muninn`.
- **S9 — Close (R9, R13, AC-R10, AC-R12, AC-R13).** Suite and full AC10 sweeps on Windows and Linux; report and
  merge; then rename on GitHub, set `origin`, copy the memories of `muninndb` into `krummi`, and a final commit with
  their outcome in the report.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1 | inline | pending | |
| T2 | S2, S3, S4 | inline | pending | |
| T3 | S6 | inline | pending | |
| T4 | S5 | inline | pending | |
| T5 | S7 | delegated (Writer trigger (a): independent of T1–T4, disjoint surface `docs/prd/**`, long careful edit) | pending | |
| T6 | S8, S9 | inline | pending | |

Risk: T2 touches item 1 (persisted format change), so it gets its own commit, an independent verifier and an RDD
review.

## Log

- **L1** (2026-10-23, user, verbatim): "Implementa" — authorization: "Autorizo los dos" (GitHub rename of
  jesuslpm/MuninnDB to jesuslpm/Krummi with the user's gh session and `origin` repointed, no push; Engram: read
  `muninndb`, save copies in `krummi`, delete nothing); estimate 1 h 15 min – 2 h, answer "Sí".
