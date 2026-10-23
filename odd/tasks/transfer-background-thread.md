# Transfer background thread — ODD execution document

Objective: committed versions reach `krummi.data` within a bound with no further commits, and data fsyncs happen
only when they release a journal file (J5 of `docs/prd/krummi/journal.prd.md`; bug fix, no PRD).
Branch: `fix/transfer-background-thread`. Delivery: `ask-on-risk`. Test runner: `cargo test --workspace` (Windows).
Estimate: 1 h 30 min – 2 h 30 min. Real start: 2026-10-23 18:10:51 +0100.

## Specs

- **S1 — No time limit in the scratch area (bug 1, J5).** Versions leave the scratch area within a bound with no
  further commits: after a read that blocked the transfer ends; after the last commit was deferred and its group
  became durable; and when a commit published while a running transfer had already built its pending list.
- **S2 — One background transfer thread.** It owns every transfer (L6: "el hilo de traslado debería ser el único
  que traslade"): a writer that needs room (T14) and the test hooks ask it and wait; only close and drop, once the
  thread has ended, write versions themselves. "A commit, the end of a read, and a journal
  group becoming durable each set a flag and notify a condvar. None of them does I/O." Signals during a pass
  collapse into one more pass; a pass waits for a minimum interval or a scratch byte threshold.
- **S3 — Data fsync only when it releases a file (bug 2).** "Fsync `krummi.data` and the header only when `applied`
  reaches the last txid of the oldest old journal file. Write both header copies in the same pass", with a minimum
  interval between data fsyncs. The release decision stays in `Journal::remove_files`.
- **S4 — Unchanged.** Close (`flush_all`) still transfers everything, and so does dropping a store without a close.
  A transfer failure still makes the store refuse writes, which the next operation reports (J13).
- **S5 — Values (user, L2–L4).** Pass interval 200 ms, scratch threshold 32 MiB, data fsync interval 2 s; internal
  constants with test hooks, no public API change.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1, S2, S3, S4, S5 | inline | done | see Log L7 |
| T2 | S3, S5 | inline | merged into T1 | — |

Route evidence: one module (`krummi-pager` scratch area and journal) plus test hooks in `krummi-tx`; understood; no
Writer trigger reason. Risk: item 4 (a new thread, condvar ordering, shutdown): independent verifier at the end.

## Log

- **L1** (2026-10-23, user, verbatim): the request "Fix: committed versions stuck in the scratch area, and useless
  data fsyncs" (bugs 1 and 2, decided design, tests 1–7, risk item 4), plus "Háblame en español".
- **L2** (user): interval and threshold — "200 ms y 32 MiB".
- **L3** (user): data fsync interval — "2 segundos entre fsync".
- **L4** (user): exposure — "Constantes internas (Recommended)". Estimate 1 h 30 min – 2 h 30 min, answer "Sí".
- **L5** (decisions taken during the run):
  - The transfer thread starts before recovery and receives the store through a channel, so failing to start it
    leaves the store untouched, as E13 requires of the journal's writer.
  - The test transfer hook moves after the pending list is built and before the page writes, so test 3 can hold a
    transfer that already built its list; AC-J17 and AC-J9 keep holding it before the writes.
  - `transfer_now` (tests only) ignores the data fsync interval; since L6 it asks the thread for the pass.
  - `Scratch::new` (unit tests of the scratch area) starts no thread; `Scratch::open` (every store) does.
  - S3 of journal-followups asserted that 400 deferred commits keep under 2 MiB in the scratch area; with a 32 MiB
    threshold that bound no longer holds by default, so that test lowers the threshold through the test hook.
  - A hook `set_transfer_paused` stops the thread's own passes (it still serves demands), for tests whose
    injected faults are meant for another thread: AC-E11 and AC-E17.
  - The transfer hook is called by every flush, forced ones included, so a test can check which thread transfers.
- **L6** (2026-10-23, user, verbatim, during T1): "el hilo de traslado debería ser el único que traslade. ¿no?" —
  verified: the writer's flush in `reserve` (T14), `transfer_now` and `checkpoint` ran on the caller's thread. Now
  they send a demand to the thread and wait for its outcome; without a thread (`Scratch::new`) or after it ended,
  the caller does it, being the only one. Pinned by `only_the_transfer_thread_transfers` (fails if the writer's
  flush runs inline).
- **L7** (T1 evidence): RED observed for tests 1–5 on the old code (tests 1–3 timed out in `wait_until`; test 4:
  187 data flushes after 200 commits against 37 after 50; test 5: one pass per commit). GREEN after the change;
  `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test --workspace`
  194 passed twice, and once more with `KRUMMI_JOURNAL_BUFFERED=1`. T2 merged into T1: the release path was
  rewritten in the same restructure, and splitting it would leave an intermediate fsync policy nobody asked for.
  Adapted existing tests:
  - AC-E17 failed the transfer through a data fsync that released nothing (bug 2); it now fails the page write the
    transfer does make (`Op::Write`), with the thread paused.
  - AC-E11 pauses the thread so the injected write failure reaches the journal's writer.
  - AC-J9 waits for the store to refuse writes, since the transfer fails on its thread after the commit returns.
  - AC-J12 waits until the old journal files are released (both header copies written) before the read.
  - AC-J4 drops its first store before the next recordings: its transfer thread shares the test's recording.
  - S3 of journal-followups lowers the threshold to 256 KiB (see L5).
- **L8** (independent verifier, commit `6709631`): S1, S2, S3, S5 met; S4 met for I/O failures. Blocker B1: a
  transfer failing with a class that leaves the store accepting writes (a full disk, out of memory, access) made the
  thread stop and the store refuse writes for good; the baseline retried on the next commit. Fixed: only a failure
  that makes the store refuse writes ends the passes and is reported (J13); test
  `a_transfer_failing_without_refusing_writes_is_retried` (RED: the next commit failed with the I/O class; GREEN
  after the fix). Suite 195 passed; fmt and clippy clean. Advisories kept: A1 `Pager::data_syncs` is public without
  `test-hooks`, like the other "(tests)" methods of the crate; A2 after a panic of the thread, demands run on the
  caller and passes stop until the close (out of domain); A3 with over 32 MiB held by a long read every commit
  starts a pass, no more than the baseline's one transfer per commit; A4 a stale `retry_at` after a demand costs
  one extra pass. Verifier probes (6 writers, 3 readers, close/drop/reopen) passed.
- **L9** (RDD review, 2026-10-23): commits `6709631`, `ae06585`, `74ab5d1` (phase log) against `c075d29`; lenses
  risk, resilience, reliability; result approved, acknowledged (lineage `review-fec7abde0abc0132`, authority
  burned). Findings, both WARNING and informational, same cause: R3-retry-at-busy-loop (reliability) and
  R4-stale-retry-spin (resilience) at `transfer.rs:308-316`: after a pass fails without refusing writes, a due
  put-off release made the thread run failing passes back to back. Resolution (follow-up commit): a failure drops
  the put-off release until the next signal, and a put-off release also waits for the pass interval. Test
  `a_failing_pass_does_not_spin_on_a_due_release` (RED: 15,617 and 17,798 passes in 1.5 s; GREEN). Suite 196
  passed; fmt and clippy clean.
