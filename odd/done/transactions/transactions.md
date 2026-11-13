# Transactions — ODD execution document

Objective: implement module 2 of MuninnDB, the transactions, as specified in `docs/prd/muninndb/transactions.prd.md`
(v1.1) and its ancestry `docs/prd/muninndb/pager.prd.md` (v1.0) and `docs/prd/muninndb/muninndb.prd.md` (v1.1).
Branch: `feat/transactions`. Delivery: `ask-on-risk`. Test runner: `cargo test --workspace` (Windows; Linux in CI).
Estimate: 1.5–3 h. Real start: 2026-10-22 19:18:22 +0100.

## Specs

The PRD is authoritative; every S# names its PRD items, and the PRD text of those items is the spec.

- **S1 — Public frame (T1, T8, T18, Salida).** "abrir una transacción de lectura, abrir una de escritura (con un
  límite de espera opcional, T4), confirmar una de escritura y abortarla". Page access stays internal. Closing the
  store with open transactions, reusing a finished transaction, using a page reference after its transaction ends
  or after a change of the same write: does not compile. New error classes Timeout and Misuse carry the folder or
  temporary, and Timeout the requested limit.
- **S2 — Snapshots and visibility (T2, T16, AC6, AC20, AC-T3, AC-T4, AC-T5, AC-T15).** "Una de escritura ve lo
  confirmado hasta el momento en que obtiene su turno"; a commit is visible to every transaction opened after it
  returns; a read opened during a commit sees all or nothing. Reads return references into the mapping, the
  temporary store memory or the scratch version, never a copy; same address for the same version.
- **S3 — Writer queue (T3, T4, T5, T13, AC7, AC-T6..AC-T9, AC-T18).** Strict FIFO, no barging; optional limit;
  zero limit fails with Timeout if a write is open or anyone is queued; a limit that cannot be counted fails with
  Misuse, 24 h always accepted; second write on the same thread fails with Misuse without waiting; precedence
  "uso incorrecto (T4, T5), entrada y salida (T12), tiempo de espera agotado con límite cero (T4)".
- **S4 — Threads and readers (T6, T7, AC-T10, AC-T12).** Read transactions Send + Sync; write transactions neither
  (compile-fail). No reader limit; out of memory when opening a read fails with that class.
- **S5 — Commit, abort, failures (T9, T10, T11, T12, AC11, AC12, AC-T1, AC-T2, AC-T13, AC-T14, AC-T17, AC-T19,
  AC-T20).** Committed changes reach `muninn.data` at the latest on close or drop; abort, drop and panic apply
  nothing; a failed operation makes the commit fail with the first class and end the transaction; an I/O
  failure on write makes waiters and new requests fail at once with I/O; close retries the flush; an empty
  commit always succeeds.
- **S6 — Scratch and maximum size (T14, T15, T17, AC-T16, AC-T21, AC14, AC10).** Each page keeps its number; the
  committed version lives in scratch memory while a read needs the older one, then is written in place. File +
  scratch + open write count toward the maximum; the space comes back when the reads end. The header is never
  rewritten. Temporary mode behaves the same and creates no files.
- **S7 — Platforms and unsafe policy (AC18, AC21).** Tests pass on Windows and Linux; unsafe code only in the
  Pager crate.

## Design (decided before the run)

- **Pager crate (`muninndb-pager`)**: new `scratch` module, Voron's scratch area. `Scratch` wraps the `Pager`,
  keeps committed page versions (`Arc` sealed pages) by page number, live snapshots (sequence + version) and
  retired versions. `Snapshot` (Send + Sync) resolves a page to its newest version not newer than the snapshot,
  else to the mapping. `ScratchWriter` (one at a time, internal mutex) publishes a commit atomically under the
  state lock and flushes versions in place when no live snapshot is older than them, with I/O outside the lock.
  Retired versions are freed when every snapshot that could hold a reference has ended. New error classes
  `Timeout` and `Misuse`. A `test-hooks` feature exposes fault injection to the other crates' tests. The memory
  storage switches to raw allocations so a flush can write one page while other pages are borrowed.
- **New crate `muninndb-tx`** (no unsafe): the writer queue (FIFO tickets, limits, owner thread, I/O state), the
  read and write transactions with the internal page API (`page`, `page_mut`, `allocate`), the meta page at
  page 2 (next free page; a blank page means a new store), the maximum-size accounting with flush on demand, and
  commit hooks for the tests (`test-hooks` feature).
- **Facade (`muninndb`)**: `Store::read`, `Store::write`, `Store::write_with_timeout`, `WriteTransaction::commit`
  and `abort`; close and drop flush the scratch.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S2, S6 (pager side), error classes | inline | done | `9398106`, fix `f87b5d9` |
| T2 | S2–S6 (transaction layer) | inline | done | `0aab615`, test fix `1632e39` |
| T3 | S1, S4, S7 (public API, compile-fail, docs) | inline | done | `c97af52` |
| T4 | Acceptance: full AC10 sweeps, Linux run (AC18), CI sweep | inline | done | `ce893ab` |
| F1 | S6: verifier blocker B1 (unreadable versions retired, not freed) | inline | done | `610bc40`, tests `c88d1fa` |

Test-first exception: T1–T3 were written code first and tests right after, in the same task, not RED first; every
acceptance criterion has its test (map below).

| Criterion | Test |
|---|---|
| AC6 | `muninndb-tx/tests/commit.rs` `ac6_*` |
| AC7, AC-T7 | `queue.rs` `ac7_concurrent_*`, `ac7_ac_t7_*` |
| AC10 | `commit.rs` `ac10_sample_*` (suite), `ac10_every_altered_byte` (acceptance, CI) |
| AC11 | `commit.rs` `ac11_*` |
| AC12, AC-T20 | `commit.rs` `ac12_ac_t20_*`; scratch unit tests `a_failed_flush_*`, `close_reports_*` |
| AC14 | `commit.rs` `ac14_*` |
| AC18 | the whole suite on Windows and on Linux (WSL; CI on both) |
| AC20, AC-T15 | `commit.rs` `ac20_ac_t15_*`; `muninndb-tx/tests/ui/page_after_*` |
| AC21 | `muninndb/tests/unsafe_policy.rs` (the new crate inherits the workspace lints) |
| AC-T1 | `commit.rs` `ac_t1_*` |
| AC-T2 | `commit.rs` `ac_t2_*` |
| AC-T3, AC-T4, AC-T5 | `commit.rs` `ac_t3_*`, `ac_t4_*`, `ac_t5_*` |
| AC-T6, AC-T8, AC-T9 | `queue.rs` `ac_t6_*`, `ac_t8_*`, `ac_t9_*`; `muninndb/tests/transactions.rs` |
| AC-T10 | `muninndb-tx/tests/threads.rs`; `muninndb/tests/transactions.rs` (static Send/Sync checks) |
| AC-T11 | `muninndb/tests/ui/*`, `muninndb-tx/tests/ui/page_after_change.rs` |
| AC-T12, AC-T13 | `commit.rs` `ac_t12_*`, `ac_t13_*` |
| AC-T14, AC-T18, AC-T22 | `queue.rs` `ac_t14_*`, `ac_t18_*`, `ac_t22_*` |
| AC-T16, AC-T17, AC-T19, AC-T21 | `commit.rs` `ac_t16_*`, `ac_t17_*`, `ac_t19_*`, `ac_t21_*` |

Route evidence: one behavior chain where each unit depends on the previous one, so no parallel writers; no
writer trigger fires and the parent works inline.

## Log

- **L1** (2026-10-22, user): "Implementa tansactions.prd.md con ODD"
- **L2** (2026-10-22, before the start): T14 and AC-T16 corrected to Voron's design (PRD v1.1, user's choice
  "Corregir el PRD"); scratch stays in process memory in module 2, files arrive with module 3 (user's choice
  "Memoria ahora, archivos en mód. 3").
- **L3** Assumption: the meta page (page 2) is a data page, not part of the header, so T17 holds and the format
  version stays `1`; a module-1 store has a blank page 2 and opens as an empty transaction store.
- **L4** Assumption: "toda escritura que necesite más espacio" accounting counts `muninn.data` length + scratch
  versions + the open write's dirty pages; before failing with Maximum size, the writer flushes what no read
  needs (that is how the space "se recupera cuando terminan las lecturas").
- **L5** RDD review 1 (T1, commits `9398106` + `8ee4c66`, base `ab1223c`): lenses risk, resilience, reliability;
  approved; lineage `review-20ce3d3cb04d18df`. Findings, all non-blocking: R3-writer-sync (WARNING, `ScratchWriter`
  was `Sync`, so safe code could run two flushes at once) → fixed in `f87b5d9`; R4-001 (WARNING, `allow_writes_again`
  also clears a refusal caused by a failed sync) → accepted: nothing syncs before close in module 2, and close
  syncs again and reports it; R3-limit-untested and R3-new-api-untested (WARNING/SUGGESTION, new pager APIs without
  pager-level tests) → covered by the transaction tests (max size, blank meta page).
- **L6** RDD review 2 (T2 slice, commits `f87b5d9` + `0aab615`, base `8ee4c66`): lenses risk, resilience,
  reliability; approved; lineage `review-8a0f199c928b52a3`. Findings, all non-blocking: R3-queue-order-race and
  R3-queue-expiry-hang (WARNING, timing in the ordering test) → fixed in `1632e39`; R3-handoff-c-not-proven-queued
  (SUGGESTION) → fixed in `1632e39`; R3-send-sync-contract-untested (SUGGESTION) → static checks in `c97af52`;
  R4-temp-store-refusal-empty-folder (SUGGESTION, the I/O error of a temporary store has an empty folder) →
  accepted: `Error::Io` has carried an empty folder for temporary stores since module 1.
- **L7** Default taken during the run: negative Send/Sync checks are static assertions, not trybuild tests, because
  the compiler prints `Sync` or `std::marker::Sync` depending on the features unified in the build (it failed under
  `cargo test --workspace`).
- **L8** RDD review 3 (last slice, commits `1632e39`, `c97af52`, `ce893ab`, base `0aab615`): lenses readability,
  reliability; approved; lineage `review-5b0933ca6fbb647a`. Findings, all non-blocking: R2-commit-twice-name and
  R2-unused-thread-return (SUGGESTION) → fixed in `c88d1fa`; R2-dup-negative-trait-probes (SUGGESTION, the
  NotSend/NotSync probe traits are copied in three crates) → accepted, one copy per crate, each next to its use;
  R3-trybuild-stderr-toolchain-sensitive (WARNING) → accepted: CI pins Rust 1.99.0, as for the Pager's compile-fail
  test.
- **L9** Acceptance (Linux, WSL Ubuntu, Rust 1.99.0): `cargo test --workspace` all green; full AC10 sweeps green:
  Pager 69.56 s, transactions 24.31 s.
- **L10** Independent verifier (Sonnet, high): S1–S5 and S7 met, S6 partly. Blocker B1: `collect_garbage` retired
  the versions nobody can read instead of freeing them, so a long read plus 3000 rewrites of one page held about
  24 MiB outside the maximum. Fixed in `610bc40` (RED observed first with
  `rewriting_a_page_under_a_long_snapshot_keeps_memory_bounded`); the verifier's probe now shows 16 KiB extra, and 0
  after the read ends. Advisories: A1 the sealed copy made at commit is not counted (transient, up to the write
  set) → accepted; A2 a second read of the same page in one transaction can change address after an in-place
  flush (same bytes; held references keep theirs, AC20 asks it only without intermediate commits) → accepted;
  A3 → covered by the new test; A4 a flush failure inside an operation counts as that transaction's first failure
  → consistent with T11 and T12.
- **L11** Default taken during the run: AC-T16 ("llega un momento en que una escritura falla") is read as a store
  whose maximum leaves no room for the versions a long read retains. With Voron's design (T14 v1.1) a read retains
  at most one extra version per page, so rewriting the same pages does not grow memory without limit; the
  verifier's probe expected a failure with any maximum. Worth an annotation to AC-T16 if the user agrees.
- **L12** RDD review 4 (fix slice, commits `610bc40`, `c88d1fa`, base `ce893ab`; assessed `under_budget`, reviewed
  because it closes the run): lenses risk, reliability; approved; lineage `review-e0c7e39c5e00b78f`. Findings, both
  non-blocking: R3-bytes-accounting-moved (WARNING, the byte counter could drop twice) → checked: `Version` has no
  `Drop` and the freed versions no longer go through `retire`, so each page is subtracted once;
  R3-intermediate-snapshot-coverage (SUGGESTION) → already covered by `a_version_an_open_snapshot_reads_is_kept`,
  which runs the same garbage collection with a middle snapshot open.

## Closing

Closing report: [transactions-report.md](transactions-report.md).
