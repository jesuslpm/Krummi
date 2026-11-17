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
| T1 | S1, S5 | inline diagnosis, writer if a change follows | pending | |
| T2 | S2, S5 | delegated writer (context backstop) | pending | |
| T3 | S3, S5 | inline exploration; PRD draft if bytes change | pending | |
| T4 | S4, S5 | WSL profile first | pending | |
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
