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
| T1 | S1 | inline | done | `59d673a` |
| T2 | S2 | inline | done | `a9baf5d` |
| T3 | S3 | inline | done | `46d64c8` |
| T4 | S4, S5 | inline | done | `908a069` |

Route evidence: small units of one module, each understood; no Writer trigger reason.

## Log

- **L1** (2026-10-23, user, verbatim): "Implementa lo pendiente" — scope answer: "Barridos y avisos útiles
  (Recommended)"; estimate 1 h 30 min – 3 h, answer "Sí".
- **L2** T1: a close of an unchanged store writes nothing (RED: the close rewrote both header copies); test-only
  `testing::skip_syncs` in the transactions and journal AC10 sweeps. Measured on Windows: the samples did not get
  faster (open after rewriting the file costs about 54 ms, the antivirus scanning it; open plus close alone
  0.4 ms), so the gain is expected on Linux, where the flushes dominated.
- **L3** T2: recovery reads one record at a time (no runnable RED for memory; the 24 recovery tests cover the
  behaviour). T3: a transfer makes the journal durable once 1 MiB or 256 commits are pending (RED: 3.2 MiB waited
  in the temporary area). T4: chunks of at most 8 MiB (RED: a 16 MiB chunk); AC-J7 close branch now power-cuts
  under the three models.
- **L4** User question during the run: direct I/O and page diffs as in Voron. Verified in Voron's source
  (`WriteAheadJournal.cs`: `ComputeDiff` when the transaction is compressed and not encrypted; the O_DIRECT
  durability probe in `StorageEnvironmentOptions.cs`). MuninnDB writes through the page cache plus `fsync` and
  journals full pages. Proposed as a separate change with its own PRD, after the comparative bench (module 6):
  direct I/O changes what deferred commits survive, and diffs need the transfer never to write a page newer than
  the header records.
- **L5** RDD review, recorded when it ended:

  | Commits | Lenses | Result | Findings and resolution | Lineage |
  |---|---|---|---|---|
  | `8168fa1..63e1d5c` | risk, resilience, reliability | approved | Advisories only: WARNING R3-close-unchanged-predicate (checked: a commit with changes always creates a journal file, removed only by close or recovery, so the predicate holds only without commits; kept); SUGGESTION R3-j5-ordering-unasserted, R3-s2-memory-bound-untested, R3-skip-syncs-not-reset, R4-forced-flush-on-transfer, R4-recovery-double-read, R4-smaller-temp-chunks-more-files (accepted). | `review-c0a3d835b18c9657` |
- **L6** User question: CanopyDB's WAL. Verified in its source (`src/wal.rs`, `src/utils.rs`): `use_direct_io` is an
  option, `false` by default, `O_DIRECT | O_DSYNC` on Linux and Android only, with a support probe and fallback;
  the default path writes through the page cache and calls `sync_data`. Noted for the direct I/O proposal: use
  `sync_data` on preallocated journal files.
- **L7** Independent verification (Sonnet, high): S1–S5 met, no blockers; checks green; differential probes
  against `8168fa1` identical (616 + 122 damage and power-cut outcomes; 4,245 deferred power-cut cases, 0 bad).
  Advisories:
  - Fixed in `ecfbb36` (RED observed: the store became unopenable): a close without changes no longer repaired a
    damaged header copy; now it rewrites the header when a copy was damaged at the opening.
  - Accepted: a read failure in recovery's second pass can leave `muninn.data` partly written; the journal stays
    and the next opening repeats recovery with the exact result (J8 allows it).
  - Default chosen without asking: the deferred commit that reaches 256 pending commits or 1 MiB waits for the
    flush before returning, against the letter of J10 ("devuelve sin esperar a ser durable"). Documented in the
    API (`ecfbb36`); resolving it needs a separate thread, which the proposed grouped-write PRD brings.
  - `f567b8f`: the larger Pager made clippy flag `Created`; the pager is boxed.
- **L8** Proposals from the user's questions (for a future PRD "Diario: escritura agrupada y directa"):
  preallocated journal files with `sync_data`; direct I/O (`O_DIRECT | O_DSYNC`, write-through) with a fallback;
  group commit through a journal thread that writes queued commits in one write. User decision (verbatim):
  "Commit debería esperar a que todo esté durable, pero primero suelta el turno de transacciones." Open: what
  the next writer and the readers see of queued commits (recommendation: the writer sees them, readers only
  durable ones). Deferred commits stay (recommendation), redefined as queued without waiting.
- **L9** Journal sweep: reusing one folder did not help on Linux (1,955 s); profiling showed 1.4 ms per attempt.
  Cause: the sweep enabled `skip_syncs` before recording its run, and recording resets the thread's test state.
  Fixed in `0574433`; the full sweep now takes 158 s on Linux.
- **L10** RDD review of the last slice, recorded when it ended:

  | Commits | Lenses | Result | Findings and resolution | Lineage |
  |---|---|---|---|---|
  | `63e1d5c..0574433` | resilience, reliability | approved | SUGGESTION R3-001 on the new header-repair test (`crates/muninndb-tx/tests/journal.rs:306-329`); accepted. | `review-49ee8fc8ea327cbb` |
- **L11** User decision (verbatim) on the product name, for a separate renaming change with its own PRD: "Pues
  Krummi sin suijo detrás." (MuninnDB is taken by an AI-memory database at muninndb.com; `krummi` is free on
  crates.io.)
- **L12** Final Linux acceptance on `0574433`: suite green (64 s); full AC10 sweeps green: Pager 54 s, transactions
  27 s (was 1,362 s), journal 158 s (was 3,666 s).

## Closing

Closing report: [journal-followups-report.md](journal-followups-report.md).
