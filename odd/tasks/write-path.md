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
| T1 | S1–S3 | delegated writer (long-session backstop): O5 | done | `6794ebc`, `6e32369`, `f5e1d1c`, L6, L7 |
| T2 | S4 (S1 of the PRD) | delegated writer (long-session backstop): O6 | done | `f7b19d8`, L8 |
| T3 | S4 (S2, S3 of the PRD) | delegated writer (long-session backstop): O7 | done | `2b11374`, `1ac73e3`, L10, L11 |
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
- **L6** (2026-11-06): T1, O5, commit `6794ebc`.
  - Removed:
    - `stage` seals each payload straight into its temporary-area slot (`TempArea::store_with`, new unsafe code in
      the Pager). This drops `SealedPage`, `alloc_sealed` and one zeroed 8 KiB allocation and copy per page.
    - A record alone in its group's write is written from its own buffer, with its header's `durable_upto` rewritten
      in place. The no-memory fallback does the same.
    - `lz4_flex` drops `safe-encode` (D12 annotation).
  - Kept:
    - The raw payload copy: LZ4 compresses one contiguous block, so its matches span entries.
    - The record copy: compressing into the record would hold more memory than the queue counts.
    - The group copy when several records share a write: in-place layout across threads is a concurrency redesign,
      and vectored writes change the recorded writes.
  - New tests: `bytes_tests::commits_keep_the_bytes_of_the_journal_and_the_data_file`, recorded on the base, and
    `temp::tests::slots_filled_in_place_hold_what_was_written_and_a_panic_holds_nothing`.
  - Checks: fmt ok, clippy clean, 548 passed, 0 failed, 30 ignored (parent rerun).
- **L7** (2026-11-06): independent verifier of O5 (risk items 1 and 4): PASS, no blockers.
  - The journal writes are identical to the base over 120 mixed rounds: 126 writes, same offsets, sizes and hash.
  - Pages 3 and after of `krummi.data` are identical. Pages 0 and 2 vary by about 9 bytes between runs on both
    trees.
  - `store_with`'s SAFETY comments hold.
  - A stage that fails mid-way frees its slots and publishes nothing.
  - A lone record's header rewrite is idempotent on retry.
  - Advisories:
    - A1: `krummi-pager` no longer exports `SealedPage` or `alloc_sealed`. These are internal crates.
    - A2: the pinned data-file hash could flake on the varying pages. Fixed in `6e32369` and `f5e1d1c`: the test
      hashes every page but 0 and 2. The pinned value was computed both on the base (the verifier's copy of
      `b9514cf`) and on the change, 3 runs each, and was the same.
- **L8** (2026-11-06): T2, O6, commit `f7b19d8`.
  - Each page in the transaction's `dirty` map carries a `checked` flag (`AtomicBool`). `payload_mut()` clears it;
    it is the only path to mutable bytes. New entries (`copy_for_write`, `add_zeroed`) start with it cleared.
  - `PageSource::page_and_code` becomes `page_and_check`: `Verified(code)` uses the shared memory, `Changed {
    checked }` never touches it, and `Unverified` (temp area) is checked on every read.
  - Tests: AC-S1, AC-S2, AC-S3 and AC-S7's layout part (`crates/krummi/src/writer_check_tests.rs`), on disk and in
    temporary mode.
    - RED first: AC-S1 and AC-S3 failed before the skip.
    - Mutation: when `payload_mut` stops clearing the flag, AC-S1 and AC-S2 fail.
  - The flag is an atomic, not a `Cell`, so the compile-fail `.stderr` files keep their text.
  - Checks: fmt ok, clippy clean, 552 passed, 0 failed, 30 ignored.
- **L9** (2026-11-06): RDD reviews.
  - `b9514cf..6794ebc` (O5): escalated by item 4. Lenses risk, resilience and reliability. Approved, lineage
    `review-1bb41836c2a9b9bc`. Findings:
    - R1-001 (warning): the unsafe LZ4 encoder. Accepted: the user's decision, D12 annotation.
    - R3-recording-global-filter (warning): dismissed. The recording state is per thread (`sys/faults.rs:48`), and
      engine threads only adopt the opening thread's state.
    - R3-direct-write-alignment (warning): dismissed. `Queued.record` is an `AlignedBuf` (the verifier checked it).
    - R3-reused-slot-fill (suggestion): covered by the reused-slot test in `temp.rs` and by `seal_page` writing
      every byte.
  - `6794ebc..6e32369` (O6 and the test fix): lenses risk and reliability. Approved, lineage
    `review-088e5f8ba4d50d7c`. Findings:
    - R1-001 and R3-bytes-pin-weakened (suggestion, warning): the pin now keeps page 1 as well (`f5e1d1c`). The
      value is computed on base and change alike.
    - R3-s1-no-merge-coverage (suggestion): handed to T3's writer, which adds a delete-and-merge case to AC-S1.
- **L10** (2026-11-06): T3, O7, commit `2b11374`.
  - Every store read of a write transaction goes through `Snapshot::page_and_code` or `Snapshot::run`, and now uses
    `Integrity::UseRemembered`: tree descent, copy for write, free-space map, meta page (`slot`) and value runs.
    `CheckAndRemember` and `Snapshot::integrity()` are removed. Opening still uses `Check`.
  - Tests replaced:
    - `ac_i2_a_write_checks_every_page_it_reads` → two AC-S4 tests.
    - `ac_i8_a_write_detects_a_remembered_page_damaged_while_open` → `ac_s6_a_write_uses_a_remembered_leaf_altered_while_open`.
    - The fixed-size AC-I8 test (the fourth part of AC-X19) → `ac_s6_a_write_uses_a_remembered_fixed_size_leaf_altered_while_open`.
  - Tests added: AC-S5, and an AC-S1 case with merges and page reuse that passes at once, as expected.
  - RED: AC-S4 failed (127 checks where 0 were expected), and both AC-S6 failed (damage).
  - README updated.
  - Checks: fmt ok, clippy clean, 555 passed, 0 failed, 30 ignored (parent rerun).
  - `1ac73e3` clarifies a test comment (RDD suggestion).
- **L11** (2026-11-06): reviews of O7.
  - RDD of `6e32369..2b11374`: lenses risk and reliability. Approved, lineage `review-ef52b399f8a44a80`.
    - R1-001 (warning) is the residual S3 accepts.
    - R3-bytes-pin-comment-ambiguous (suggestion) fixed in `1ac73e3`.
  - Independent verifier of O6 and O7 (risk items 2 and 6): PASS, no blockers.
    - The whole suite plus a probe that hashes a changed page's bytes at each passed check and asserts them on every
      trusted read: 0 violations. Its negative control (the flag no longer cleared) fires at once.
    - S2 probes:
      - A rewritten page damaged while open fails on every writer read and refuses commit.
      - A page read by an abandoned write stays remembered.
    - Differential on a fixed workload: `6794ebc`, `6e32369` and `2b11374` give identical results, journal writes and
      data-file pages (all but 0 and 2).
    - Advisories:
      - A3: S2 does not name the meta page. The writer now reads it like readers do, and opening still checks it.
      - A4: AC-S1 asserts against the transaction's own event log, but the hash probe covers the same invariant
        independently.
