# Growth within the maximum size without pending versions — ODD execution document

Objective: fix the intermittent `ac_f18_the_maximum_size_in_a_temporary_store` (`krummi-tx`), a bug fix with no
product change (no PRD): rule J7 of `docs/prd/krummi/journal.prd.md` ("una escritura nunca falla por tamaño máximo a
causa de versiones que ninguna lectura necesita: antes se trasladan") and T14 of the transactions PRD.
Branch: `fix/max-size-growth` (from `main` `990689b`). Test runner: `cargo test --workspace`. Estimate: 1 h – 1 h 45
min. Real start: 2026-10-26 18:57:10 +0000.

## Specs

- **S1** — J7: "una escritura nunca falla por tamaño máximo a causa de versiones que ninguna lectura necesita:
  antes se trasladan". The store's growth must not depend on whether versions no read needs were still waiting
  for the background transfer when it grew.
- **S2** — `ac_f18_the_maximum_size_in_a_temporary_store` (AC-F11 first part, AC-F18) passes under load, every
  time: "in a temporary store whose maximum size leaves room for the versions of a write but not for growth, the
  free pages are reserved and one more fails".

## Design

- **D1** — Diagnosis (explorer, spot-checked): `WriteTransaction::reserve` (`crates/krummi-tx/src/transaction.rs`)
  caps a growth at `max_size - memory`, and `memory` includes `scratch_bytes()`, the versions waiting for the
  background transfer at that moment. With some pending, the store grows to less than doubling allows and stops
  short of its full room (for example 129 pages instead of 128 + room); it never shrinks, so later writes need one
  page more than the maximum, and the flush before failing has nothing left to move.
- **D2** — Fix: before a growth, when the scratch area holds versions, transfer the ones no read needs (the same
  synchronous flush the J7 path uses), then size the growth. At most one flush per reservation.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1, S2 | inline | done | `fa77b47` |
| T2 | checks, verification, RDD and Codex reviews, closing report | inline | done (Codex pending) | `5db200f`, report |

Route evidence: one understood change in one function, inline. Risk item 4 (async ordering with the transfer
thread): independent verifier at the end.

## Log

- **L1** (2026-10-26, user): "Cuando termines arregla esto: Windows: 1 falla: ac_f18_the_maximum_size_in_a_temporary_store, de krummi-tx. Esta rama no toca ese código y la prueba falla igual en la base (3 de 5 corridas en las dos). Es la intermitencia que ya anotó el informe de 5.1, así que conviene arreglarla aparte." Then "Cuando tengas el diagnóstico arranca la ejecución sin preguntar".
- **L2** (exploration, before the start): reproduced under load (four binaries at once): `MaxSize { max_size: 1097728, needed: 1105920 }` at `free_space.rs:956` and `:962`; no failure in four runs alone. Explorer diagnosis D1.
- **L3** (T1): RED, deterministic: `growth_within_the_maximum_size_ignores_versions_no_read_needs` (transfer paused,
  so every commit leaves its versions waiting while the store grows) failed with "the store took its full room",
  left 129, right 128. Fix D2 in `WriteTransaction::reserve`. GREEN: the `free_space` binary 28 passed; under load
  (four binaries at once, three rounds) 0 failures of 12, where the same load had failed 3 of 8 before. `cargo fmt
  --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test --workspace
  --no-fail-fast`: 326 passed, 0 failed, 11 ignored.
- **L4** (RDD review, `fa77b47`, base `990689b`): assessed `medium` (`under_budget`), raised to `high` with
  `--escalate-item 4` (the writer now waits for the transfer thread before growing); lenses resilience,
  reliability; lineage `review-e01b32b2e0986d01`; **approved**, acknowledged, authority burned. Advisory findings:
  - R4-sync-flush-before-growth (WARNING): growing a store with a maximum size now waits for a synchronous
    transfer, and a failure of that transfer fails the reservation. Accepted: growth doubles, so it is rare; stores
    without a maximum size are untouched; the transfer's failures follow T11 as the existing flush before failing
    does.
  - R3-reader-pinned-growth-untested (SUGGESTION): added
    `growth_within_the_maximum_size_counts_versions_a_read_needs` (a read keeps three versions while the store
    grows: the reservation fails with the maximum-size class within the maximum, and after the read ends the rest
    of the room is reserved). It also fails with the code before `fa77b47` (`MaxSize`, needed 1,122,304, after the
    read ends).
  - R3-paused-flush-assumption (SUGGESTION): the first test now asserts that no background pass ran
    (`transfer_passes` unchanged), so the room came from the writer's own transfer.
- **L5** (independent verifier, Sonnet high, on `5db200f`): **no blockers**; S1 pass, S2 pass. Code: `reserve` is the
  only place that grows a bounded store (`ensure_pages_within` has no other production caller; recovery grows
  unbounded); the writer waits for the flush holding only its turn, which the transfer thread never needs, and
  queued demands are served before the paused flag is checked. Journal rules J5, J10 and E5 allow a deferred commit
  to become durable earlier. Probes (60 runs per mode, 12 processes in parallel): the `ac_f18` scenario on
  temporary and disk stores, with deferred commits and fast pacing; a read holding three versions across growth;
  free/allocate churn with and without reader threads: all pass, the file never exceeds the maximum, and
  `MaxSize` appears only while a read keeps versions. The same probes against `990689b` fail
  (`page_count != 128`, `MaxSize` needed 1,105,920 or 1,122,304). Six copies of the `free_space` binary in parallel:
  29 passed each. Advisories: a growth now waits for a transfer (accepted in the RDD review); a test that pauses the
  journal and grows a bounded store would now block (none does; test-only). Its scratch folder was removed.
- **L6** (Codex review): not run; Codex's usage limit runs until 10:53 PM (see the trees-scan run, L17). Pending.

## Closing

Closing report: [max-size-growth-report.md](max-size-growth-report.md).
