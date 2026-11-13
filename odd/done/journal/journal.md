# Journal and recovery — ODD execution document

Objective: implement module 3 of MuninnDB, the journal and recovery, as specified in
`docs/prd/muninndb/journal.prd.md` (v1.0) and its ancestry `docs/prd/muninndb/muninndb.prd.md` (v1.1), with the
earlier modules `pager.prd.md` and `transactions.prd.md`.
Branch: `feat/journal`. Delivery: `ask-on-risk`. Test runner: `cargo test --workspace` (Windows; Linux in WSL/CI).
Estimate: 2–4 h. Real start: 2026-10-22 21:16:32 +0100.

## Specs

The PRD is authoritative; every S# names its PRD items, and the PRD text of those items is the spec.

- **S1 — Format, files and header (J2, J3, J6, AC-J1, AC-J6, AC-J16).** Format `2`; journal files
  `muninn.journal.` + 19 digits and the `muninn.temp` subfolder are reserved; "Una apertura que termina sin error
  deja en la carpeta solo `muninn.data`"; a folder without a valid `muninn.data` holding them "no es un almacén y no
  se toca nada". The header is rewritten when the transfer advances.
- **S2 — Durable and deferred commits (J1, J4, J10, J12, AC1–AC4, AC11, AC12, AC-J7, AC-J13, AC-J14).** A durable
  commit returns only when durable and is invisible until then; a deferred one returns once written, survives a
  process kill, and a crash keeps a prefix. Disk full or memory while writing the journal: "nunca aparece al
  reabrir"; other failures: I/O, store refuses writes, "entera o no está".
- **S3 — Making durable (J11, AC-J8).** Returns when every commit returned before is durable; fails at once with
  I/O on a store that refuses writes "aunque no haya nada pendiente".
- **S4 — Transfer and checkpoint (J5, J13, J15, AC-J9, AC-J17, AC-J18).** Transfer without the application
  asking, never of non-durable commits, without blocking readers or the writer; the journal does not grow without
  limit when no read prevents it; it does not count toward the maximum size. A transfer failure outside an
  operation makes the store refuse writes, reported by the next write request, commit with changes, J11 and close.
- **S5 — Temporary area in files (J7, AC-J11, AC20, AC-T15, AC-T16).** Committed versions not yet in
  `muninn.data` live in files of `muninn.temp`, referenced without copying; deleted on close and by the next open.
- **S6 — Recovery (J8, J9, AC5, AC10, AC-J2–AC-J5, AC-J10, AC-J12).** Applies the commits not recorded as
  transferred, in order; "durable según el diario" decides between discard and damage; damage, then access, then
  the rest; grows past the maximum size; a damaged header copy still recovers everything.
- **S7 — Close and temporary mode (J14, J17, AC14).** A clean close leaves only `muninn.data`; a failed close
  leaves the journal for the next open. Temporary mode accepts deferred commits and J11 and does nothing more.
- **S8 — Compression (J16, AC-J15).** Compressible commits take less space in the journal than their pages.
- **S9 — Platforms and unsafe policy (AC18, AC21).**

## Design (decided before the run)

- **Pager crate**: everything that touches files stays here (it owns `sys` and the fault hooks).
  - `format`: format 2 header with `applied_txid` (last commit whose changes are in `muninn.data` and flushed) and
    `journal_first` (first journal file that may hold later commits). The header is written one copy at a time,
    alternating; recovery and close write both copies.
  - `temp` (new): slot storage for page versions. Disk stores: files `muninn.temp/scratch.NNNN` mapped read-write
    in chunks that never move; temporary stores: memory chunks. A version is an `Arc<Slot>`; dropping the last
    reference frees the slot, so the scratch area logic stays as in module 2.
  - `journal` (new): records aligned to 4 KiB: header (magic, txid, `durable_upto`, page count, lengths,
    checksums) and an LZ4-compressed payload of sealed pages (stored raw when it does not shrink). Files roll over
    at a size limit; before a new file is created the old one is flushed; creation flushes the folder. A failed
    write leaves the write position where it was, so the next record overwrites it.
  - `recovery` (new): pass 1 validates every record (J9 rule with `durable_upto`), pass 2 applies; then both
    header copies, delete journal files and `muninn.temp`.
  - `scratch`: commit = slots → journal record (+ flush when durable) → publish. The transfer runs at the end of
    each commit, after the turn is released, under a try-lock (`flush_lock`), only for durable versions (deferred
    ones are flushed to disk first). A checkpoint (flush `muninn.data`, header, delete files covered by both header
    copies) runs when the journal rolls over and twice on close.
- **Tx crate**: commit mode, `make_durable`, J13 refusal reporting, maximum-size accounting unchanged (versions in
  slots count).
- **Facade**: `WriteTransaction::commit_deferred` (or a mode argument), `Store::make_durable`, docs.
- **Tests**: crash by child process with `abort_at` at a `sys` point; power cut by recording `sys` operations and
  materializing what survives under three models (only flushed; plus the first unflushed write torn; plus every
  unflushed write but the first).

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1 (format, header writes), test tooling | inline | done | `3ddebc5` |
| T2 | S5 | inline | done | `0bc6c94` |
| T3 | S2, S3, S4, S7, S8 (journal, commit, transfer, close) | inline | done | `a6184cc` |
| T4 | S6 | inline | done | `8d01031` |
| T5 | S1–S4, S7 (tx and facade API, docs) | inline | done | `d6a74c9` |
| T6 | Crash, power-cut and AC10 acceptance suites | inline | done | `dd6b2af` |

Route evidence: the units depend on each other in order (format → temp area → journal → recovery → API → crash
suites), so no Writer trigger reason holds; the parent writes inline.

## Log

- **L1** (2026-10-22, user, verbatim): "Escribe el prd de Diario y recuperación, nombres en inglés, contenido en
  español e inglés" — then: "Cuando termines usa las opciones recomendadas e implementa journal.prd.md con ODD."
- **L2** (2026-10-22): PRD v1.0 committed in `f351812` with annotations to the root, Pager and Transactions PRDs.
  The user's go-ahead came with the request to implement, so the estimate (2–4 h) is recorded here instead of
  being asked; run started 21:16:32 +0100.
- **L3** T1–T6 implemented and committed; `cargo test --workspace`, fmt and clippy green on Windows after each.
  Decisions taken without asking (recorded for the closing report):
  - Format 1 headers are still decoded with their own layout, so a module-2 store is refused as an older format
    and not as damage (AC-J6).
  - The transfer runs at the end of each commit, after the turn is released, under a try-lock: a running
    transfer is never waited for (J5, AC-J17). It moves only durable versions; deferred ones wait for the next
    durable commit, J11, a journal rollover or the close.
  - The header records the transfer when the journal rolls over (16 MiB files) and twice at close and after
    recovery; journal files are removed only when both header copies are past them (J8 with a damaged copy).
  - Records are 4 KiB aligned with a `durable_upto` field: a missing or damaged commit is damage only when a
    later intact record was written after it was durable (J9). A record whose append panics is wiped (J12).
  - Test tooling: power-cut models Flushed, TornFirst, SkipFirst and Written (process death), recorded
    operation counts, a panic action, transfer and journal-sync hooks.
- **L4** RDD: the whole branch exceeded the review context budget (`lens_context_budget_exceeded`); review started
  on `f351812..5c435a4` (T1–T3) with lenses risk, resilience, reliability, lineage `review-ed0f0e4daf403899`.
- **L5** Independent verification (Sonnet, high): checks green on Windows and Linux; two blockers.
  - B1: `record_applied` advanced the header without making the journal durable; with a read held, a header
    rewrite and a deferred commit superseding durable ones, a power cut lost every returned durable commit.
    Fix: make the journal durable first. Regression test `b1_a_deferred_commit_never_makes_returned_durable_ones_disappear`
    (RED observed: page read "c0").
  - B2: a record written after a rollover kept a stale `durable_upto`, so damage in a deferred commit made durable
    by the rollover was discarded silently. Fix: re-encode the record after the roll. Regression test
    `b2_damage_in_a_commit_made_durable_by_a_rollover_is_reported` (RED observed: the open succeeded).
  - Advisory 2 fixed too: the wipe of a failed or panicked append now covers failed writes and is flushed;
    AC-J14 now runs under every power-cut model.
  - Fix commit `6e8f297`; the verifier's recheck of both blockers passed (Windows).
  - Accepted advisories: J11 waits for a commit in its flush (not for the open write); header rewrite on every
    transfer once old journal files exist (cost only); temporary chunks reserve up to 32 MiB; the max-size
    flush waits for a running transfer; the Windows power-cut model never makes removals durable; recovery
    reads whole journal files into memory.
- **L6** RDD incident: the capture launcher's glob `capture_*.sh` matched itself and re-spawned (about 6,000
  processes, killed). The first lineage `review-ed0f0e4daf403899` reached correction-required; `recover`
  moved it to `review-c3db515f25eeb4cc` over `f351812..6e8f297`, lenses risk, resilience, reliability.
- **L7** RDD reviews (recorded when they ended):

  | Commits | Lenses | Result | Findings and resolution | Lineage |
  |---|---|---|---|---|
  | `f351812..96b6f46` (T1–T6, B1/B2 fix, R1 fix) | risk, resilience, reliability | approved after one correction | CRITICAL R1-dup-txid-after-failed-sync: a journal flush failing with a lack of space left the store accepting writes and the next commit reused the txid; fixed in `96b6f46` (any failed flush is I/O and refuses writes), regression test `r1_a_journal_flush_that_fails_for_lack_of_space_refuses_writes` (RED observed: DiskFull). Advisories (WARNING/SUGGESTION): AC-J7 close branch weak, deferred versions accumulate until a durable commit, header mutex `unwrap`, recovery reads whole files, removal error class, poisoned transfer lock skips forever, failed wipe then rollover, deferred commit latency on transfer, recovery memory. Fixed: poisoned lock and failed wipe (`e500816`); the rest accepted. | `review-ed0f0e4daf403899` → recovered as `review-c3db515f25eeb4cc` |
  | `96b6f46..e500816` (advisory fixes) | resilience, reliability | approved | Advisories only: untested poisoned-lock path, positional fault injection fragile, sync-failure branch of the wipe untested, no assertion that the number is not reused; accepted. | `review-7ad40524f997b227` |

## Closing

Closing report: [journal-report.md](journal-report.md). Linux acceptance on `e500816`: suite green (81 s); full
AC10 sweeps green (Pager 51 s, transactions 1,362 s, journal 3,666 s). Active time since the start 3 h 27 min
against an estimate of 2–4 h.
