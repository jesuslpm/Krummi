# Cheaper write path: O5, O6 and O7

Objective: O5, O6 and O7 of [the performance plan](../../docs/performance/plan.md), then the complete runs. Branch
`perf/write-path` from `perf/commit-copies` `9ac34a8` (plus the agreed PRD draft). Delivery: single branch, unmerged.
Test runner: `cargo test`. O5 has no PRD (an optimization without an observable change). O6 and O7 follow
[pager-writer-checks.prd.md](../../docs/prd/krummi/pager-writer-checks.prd.md), which is authoritative for them;
its ancestry is pager-integrity-once → pager → krummi.

## Specs

- **S1** (plan, O5, verbatim): "Remove them: Seal each changed page straight into its slot of the temporary area,
  instead of into a separate sealed page. Build the journal record from those slots, without copying the raw
  payload. Write a group without copying its records into a second buffer: lay the records out in place in one
  aligned buffer, or use vectored writes. Compress with `lz4_flex`'s encoder without `safe-encode`."
- **S2** (plan, O5, invariants, verbatim): "The journal record bytes and the data file bytes stay the same as today.
  O4's golden record hashes pin the record bytes. A failure at any stage leaves the store as today."
- **S3** (plan, O5, targets, verbatim): "W2, W14, W15, W17, W19, W21."
- **S4** (PRD): decisions S1–S5 and acceptance criteria AC-S1–AC-S8 of `pager-writer-checks.prd.md`, verbatim there.
- **S5** (user, 2026-11-06): the complete runs once, at the end of O5–O7 ("Déjalo para el final"); the pending Codex
  reviews of the annotations and of the PRD are skipped ("Saltemos las revisiones de Codex"), but the Codex code
  review of this run is kept ("Sólo las que estaban pendientes").

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T0 | S4 | inline: finish the PRD (translation, annotations to other PRDs, elaboration time) | done | L5 |
| T1 | S1–S3 | delegated writer (long-session backstop): O5 | pending | |
| T2 | S4 (S1 of the PRD) | delegated writer (long-session backstop): O6 | pending | |
| T3 | S4 (S2, S3 of the PRD) | delegated writer (long-session backstop): O7 | pending | |
| T4 | S3, S4 (S5 of the PRD) | inline: single-load runs before and after each change, Windows and WSL 2 | pending | |
| T5 | S5 | inline: complete runs on Windows and WSL 2 | pending | |
| T6 | all | RDD per slice, independent verifier (risk items), Codex code review, closing | pending | |

## Log

- **L1** (2026-11-06): user's requests, verbatim: "Saltemos las revisiones de Codex, arranca con O5, O6, O7 y las
  ejecuciones completas." Then "Sólo las que estaban pendientes." about the Codex reviews. The estimate was
  "Entre 12 h y 17 h", and the user confirmed it with "Sí". Start: 2026-11-06 20:58:15 UTC.
- **L2** (2026-11-06): PRD decisions of the user. O6: option 1, a changed node is checked once after each change.
  O7: option 2, the writer uses the remembered pages. The draft was accepted ("Lo doy por bueno").
- **L3** (2026-11-06): routes.
  - The three units run one after another, not in parallel. O5 and O6 both touch the write transaction's page
    handling (`krummi-tx`), and worktrees are not used in this repository.
  - Each unit goes to one delegated writer under the long-session backstop: this session's context is already
    long. The parent keeps git, the measurements and the reviews.
- **L4** (2026-11-06): Engram mirror pending. The memory server rejects writes in this session, so this file is the
  record.
- **L5** (2026-11-06): T0. A translator wrote `pager-writer-checks.prd.en.md`. Both files have the same 14
  headings, the same S1–S5 and the same AC-S1–AC-S8 (checked by script). The annotations to the root (G6), the
  remembered page integrity, the node layout check cache and the fixed-size trees are in both languages.
  `prd_time.py` measured the whole session, which has been open since 2026-11-03 (85 h of wall clock), not this
  PRD, as with O3's PRD, so the section was left out. The PRD's own Codex review was skipped at the user's request
  (S5).
