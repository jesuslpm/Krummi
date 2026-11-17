# Night optimizations

Objective: the four optimizations left in the performance plan after the ordered-write cache run, worked without the
user overnight. Branch `feat/night-optimizations` from `feat/ordered-write-cache` (`2e45b31`, code as `main` `0cb816c`);
delivery `ask-on-risk`; tests `cargo test --workspace`. No PRD unless a change alters stored bytes or behavior (then a
PRD draft for the user, not an implementation).

## Specs

- **S1** — Candidate 8 of the plan's *Writes after O5–O7* (`docs/performance/plan.md`): «Diagnose the Windows faults on
  mapped data pages (3.5–6.3 %) with a page-fault trace before changing anything.» Without administrator rights (no
  ETW trace), measure the faults per load from the process's own counters; change something only if the diagnosis
  names a cause and an A/B shows a gain.
- **S2** — W21's copies (the plan's *Margin after the cheap gaps*): «W21 on Linux: two copies of each page (27 % in
  `memcpy`), LZ4 (15 %) and xxh3 (10 %); removing a copy is a large refactor for about 10 %, left for later.» Remove a
  copy if it can be done without changing stored bytes.
- **S3** — W17's compression (pending of the write optimizations closing report): «**La ganancia de W17 de T2:**
  pediría una regla exacta de cuándo comprimir que cumpla Y1. Queda para un PRD o un ODD aparte.» Implement only a
  rule that keeps every journal byte as today; a rule that changes stored bytes becomes a PRD draft for the user.
- **S4** — O5's group copy (plan, O5 status): «The group copy when several records share a write: removing it means
  laying records out in place across threads, a concurrency redesign. Measure its cost first.» Profile the
  concurrent write loads (W5, W6) first; change only if the profile shows a share worth it.
- **S5** — Acceptance, from the user's decision of 2026-11-17 on the ordered-write cache: «No merece la pena optimizar
  W1 si W3, W5 y W7 empeoran.» A change stays only if its target load improves and no load worsens, by the rule of Q7
  of `pager-written-pages.prd.md` (a load worsens when its median is lower and it is slower in most rounds), in an
  A/B of Krummi alone against the code before it on Windows (at least ten rounds for loads under one second per
  repetition or with several threads, three for the others; a marked load is repeated in twenty rounds).
- **S6** — Closing: complete runs (Krummi and Voron) on Windows and WSL 2 on the final code if any change stays; the
  Codex review and the closing report.

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S1, S5 | delegated diagnosis, inline change | code done, A/B pending | `d96b9c2`; RED/GREEN `growing_commits_reuse_their_buffers` |
| T2 | S2, S5 | delegated writer (context backstop) | in progress | |
| T3 | S3, S5 | inline exploration; PRD draft if bytes change | done (draft) | `929ff6b` |
| T4 | S4, S5 | WSL profile first | done, no change | profile L6 |
| T5 | S6 | inline | pending | |

## Log

- **L1** (2026-11-17) — User, after the list of the four remaining optimizations: «Me gustaría que trabajaras tú sólo en
  las 4, ya es tarde y tienes toda la noche por delante.» Start 18:42:26 (`date`), without the estimate-and-confirm
  message: the user asked to work alone. Estimate recorded only here: 8–12 h. Profilers: samply works in WSL 2
  (`perf_event_paranoid` is 1); on Windows it needs administrator rights, so Windows is measured with counters.
- **L2** (2026-11-17) — T1 diagnosis delegated (opus, high; scratch under `work/night/t1/`, no tracked edits). T3,
  read inline (`crates/krummi-pager/src/journal.rs:346-361`): the record is compressed whole with
  `lz4_flex::block::compress_into` and stored compressed only when that is smaller (`Ok(n) if n < raw_len`). No rule
  that skips compression keeps every stored byte: whether a record shrinks is known only by compressing it, an early
  abort can prove "will not shrink" only within the last 1/254 of the input (an LZ4 match encodes up to about 255
  bytes per output byte), and acceleration or partial compression change the output. So S3 cannot be done without
  changing what the journal stores (Y1 of `journal-page-diffs.prd.md`, J16): it becomes a PRD draft for the user, not
  an implementation. T2 mapping delegated to a read-only explorer (opus, medium) while T1 measures.
- **L3** (2026-11-17) — T2 map (explorer, opus medium): a W21 transaction (1,000 random fixed-size inserts) copies
  each changed page twice on the committing thread: C1, copy-on-write into a pooled `PayloadBuf`
  (`crates/krummi-tx/src/transaction.rs:704`), which isolates readers and keeps the journal's diff base, so it stays;
  C2, sealing into a fresh temp-area slot (`crates/krummi-pager/src/format.rs:239` from `ScratchWriter::stage`). C2 can
  go if the transaction's working buffer is an unpublished temp-area slot sealed in place at commit (`seal_run`'s
  layout), with the same bytes in the journal and the data file. Estimated 300–500 lines in 6–8 files; risk: item 4
  (the `Slot` immutability reasoning gains a writable, unpublished state) and item 3 (out-of-space errors would move
  from the commit to `page_mut`, and open transactions would count against a maximum size) unless a slot shortage
  falls back to today's buffer. Expected gain about 10 % of W21, some on every write load. To do after T1, delegated.
- **L4** (2026-11-17) — T3: PRD draft `docs/prd/krummi/journal/journal-skip-compression.prd.md` (v0.1, open
  questions). Reviewer (sonnet, high): fixed the draft's facts (W17's profile puts LZ4 at about 8 %, copies at 38 %;
  T2's own gain on W17 was never isolated), named the rules it would amend (J16 first point, Y1, Y6, AC-J15; D7 and Y5
  for a format change), added options (a deterministic rule without samples; value pages as literals in a valid LZ4
  stream) and the questions on growth, disk-full and determinism. Recommendation in the draft: not doing it. Codex
  review of the PRD waits for the user's answers (it runs once the draft is agreed). T3 done as a draft.
- **L5** (2026-11-17) — T1 diagnosis (worker, opus high; runs about 17:50–21:03; scratch `work/night/t1/`). Faults per
  operation counted with `GetProcessMemoryInfo.PageFaultCount` around each repetition and classified with the
  working-set watch (`InitializeProcessForWsWatch`/`GetWsChangesEx`, no administrator rights). Findings: the
  transfer's positional `WriteFile` never makes a mapped page fault again (repros over rewrites, flushes, the file
  lock); growth maps a new view and readers switch to it, so hot pages fault again after each growth (55 % of W2's
  data-file faults), but reading through the oldest covering view (e4) moved time about 1 %, not a gain under S5; the
  main cause is `encode_record_in` in `journal.rs`, which reserved the payload with `try_reserve_exact` and allocated
  each new record buffer at its exact size, so commits growing by one entry (W21: 8,206 bytes per commit, 1 to 7 MB)
  took fresh memory on almost every commit, and that churn also evicted W19's hot data pages (page 10 faulted 26
  times per repetition). e6 (geometric payload reservation, power-of-two record buffer) on scratch builds: W21 faults
  0.214 → 0.042 per op, +6.9 % and +4.7 % (0/5 and 1/5 rounds slower); W19 +4.5 % and +4.6 %; W2 +1.3 %; W3 +2.1 %;
  W14 +0.7 %; W15 +1.1 %; W17 +0.3 %; W4 +0.2 % (20 rounds); W5 −1.0 % (6/20 slower); W6 +1.1 %; W1 −0.8 % (13/20
  slower) then +1.2 % (9/20) in the 20-round repeat; no load worse under Q7. Change made inline (`d96b9c2`): `room`
  rounds a kept buffer (up to `ENCODE_RETAIN`) to the next power of two and larger ones stay exact, each allocation
  falling back to the exact size so out-of-memory fails as before; test `growing_commits_reuse_their_buffers` (RED
  observed, then GREEN). Checks: fmt clean, clippy clean, `cargo test --workspace` 869 passed, 0 failed. Risk: none
  (no stored byte changes; item 6 covered by the new test). RDD assess (`--base-ref 2e45b31 --committed-only`):
  medium, `review_due` false, `under_budget`; pending in the slice. Acceptance A/B on Windows deferred to one run with
  T2's build (`main`, `t1`, `t2`) so the machine is measured once; T2's writer (opus, high) launched in the background.
- **L6** (2026-11-17) — T4 profile on WSL 2 (samply, line tables, HEAD `2cb4db5`; script `work/night/t4/run.sh`,
  aggregates `work/night/t4/out/`). W5 (4 threads, one-entry durable commits): 4,746 on-CPU samples over four
  repetitions of about 24–26 s each; `write_group` 19.9 % inclusive, almost all of it `pwrite` (20.4 %), and the
  group copy `AlignedBuf::from_parts` 0.3 %. W6 (16 threads): 4,459 samples, repetitions of about 6.0–7.1 s;
  `write_group` 5.8 %, `pwrite` 7.3 %, `from_parts` 0.3 %. The samples are on-CPU only and the loads wait mostly on
  `fsync`, so the copy's share of the elapsed time is smaller still. Removing it would mean laying records out in
  place across threads (S4) for at most 0.3 %: not done. The T2 writer was building in parallel; the shares are of
  this process's own samples, so they stand.
- **L7** (2026-11-17) — T2 writer (opus, high) returned: `cdf0164`. A write transaction takes a free temporary-area
  slot, unpublished (`SlotMut`, `TempArea::take_free_page`, which never grows the area), as the working copy of a
  page it copies or creates (`ScratchWriter::page_buf`, `PageBuf`), and the commit seals it in place with
  `seal_run`; without a free slot the page stays in a `PayloadBuf` as before, so errors and the area's size stay at
  the commit. Tests: `crates/krummi-tx/tests/page_slots.rs` (7; RED 5 of 7 before the slot code), pager tests
  `a_page_sealed_in_its_slot_is_the_page_sealed_from_memory`, `a_page_slot_goes_back_and_fails_as_the_payload_it_stands_for`,
  `a_copy_in_a_free_slot_takes_no_payload`. Checks: fmt and clippy clean, `cargo test --workspace` 879 passed, 0
  failed; parent spot check `cargo test -p krummi-tx --test page_slots` 7 passed. Data digests of W21, W19, W2 and
  W17 equal to `t1`'s. Risk: item 4 (an unpublished writable slot state) and item 3 (internal pager/transaction
  signatures only; the `krummi` API is untouched).
  Acceptance A/B on Windows (`work/night/ab/ab.sh`, `main` = `0cb816c`, `t1` = `d96b9c2`, `t2` = `cdf0164`; 3 rounds
  for W1–W4, W14, W15, W17, W19, W21, W23–W26, W28, W29, 10 rounds for W5–W7, W12, W13, W22, W27; digests equal in
  every run). Ratios are op/s, "slower" the rounds where the newer build was slower:
  - `t1`/`main`: W21 1.025 (1/3), W17 1.208 (0/3), W19 1.017 (0/3), W2 1.021 (0/3), W1 1.027; marked (median lower,
    slower in most rounds): W23 0.980 (3/3), W12 0.856 (7/10), W13 0.826 (7/10), W5 0.998 (6/10), W22 0.993 (6/10),
    W27 0.994 (7/10), W26 0.996 (2/3), W29 0.998 (2/3). Repeated in 20 rounds (`rep1.sh`).
  - `t2`/`t1`: W21 1.052 (0/3), W19 1.061 (0/3), W2 1.045, W14 1.038, W15 1.042, W24 1.042, W28 1.034, W29 1.042, W23
    1.033 (all 0/3), W6 1.043 (0/10); worse: W7 0.976 (10/10), W3 0.972 (3/3), W26 0.993 (3/3), W17 0.966 (2/3).
    W7 and W26 only read, and the change touches no read path, so their cost points to code layout, as with the
    ordered-write cache; W3 (one bulk transaction) may pay for writing pages in mapped temporary-area memory. Under S5
    T2 does not stay as it is; a diagnosis follows once the 20-round repeats end.
