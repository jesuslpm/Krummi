# Journal follow-ups — ODD execution document

Objective: close the pending items of the journal closing report ([journal-report.md](journal-report.md)) that change
something real, without changing the product rules of `docs/prd/muninndb/journal.prd.md` (no PRD: tooling,
performance and robustness within the existing rules).
Branch: `feat/journal-followups`. Delivery: `ask-on-risk`. Test runner: `cargo test --workspace` (Windows; Linux in
WSL). Estimate: 1 h 30 min – 3 h. Real start: 2026-10-23 06:33:18 +0100.

## Specs

- **S1 — Faster full AC10 sweeps.** The full sweeps took 1,362 s (transactions) and 3,666 s (journal) on Linux,
  from seconds before module 3. A close of a store nobody changed writes nothing; the sweeps skip the flushes to
  disk they do not test.
- **S2 — Recovery without whole journal files in memory** (advisory R3/R4-recovery-loads-whole-journal). Memory of
  recovery bounded by the largest record, not by the journal's size; same decisions (J8, J9).
- **S3 — Deferred versions do not pile up** (advisory R3-deferred-versions-accumulate). Without a durable commit,
  deferred commits still reach `muninn.data` once enough of them are pending, so `muninn.temp` stays bounded (J5:
  they are made durable first).
- **S4 — Smaller temporary chunks** (verifier advisory 4). The temporary area grows in chunks of at most 8 MiB
  instead of 32 MiB.
- **S5 — AC-J7 close branch** (advisory R3-ac-j7-close-branch-untested). The test cuts the power after a clean close.

User decision (L1): the design-only advisories stay as they are (J11 waits for a commit in its flush; the Windows
power-cut model never makes removals durable).

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1 | inline | pending | |
| T2 | S2 | inline | pending | |
| T3 | S3 | inline | pending | |
| T4 | S4, S5 | inline | pending | |

Route evidence: small units of one module, each understood; no Writer trigger reason.

## Log

- **L1** (2026-10-23, user, verbatim): "Implementa lo pendiente" — scope answer: "Barridos y avisos útiles
  (Recommended)"; estimate 1 h 30 min – 3 h, answer "Sí".
