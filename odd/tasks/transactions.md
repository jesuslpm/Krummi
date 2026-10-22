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
| T1 | S2, S6 (pager side), error classes | inline | pending | |
| T2 | S2–S6 (transaction layer) | inline | pending | |
| T3 | S1, S4, S7 (public API, compile-fail, docs) | inline | pending | |
| T4 | Acceptance criteria not covered by T1–T3 | inline | pending | |

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
