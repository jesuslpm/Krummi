# Trees large values code review resolutions

Resolutions of [2026-10-27-code.md](2026-10-27-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-10-27

Fix commits: `7b4286b` (the fix and its regression tests), `3a528ec` and `c0b8d1a` (the independent verifier's
probes kept as regression tests; not part of R1). Reviewed head of the original report: `46fdd12`.

### R1 — major: Contiguous version chunks bypass the configured maximum size

- Disposition: fixed.
- Evidence: verified against the code before the fix: `TempArea::grow` (`crates/krummi-pager/src/temp.rs`) added a
  chunk of the run's own size for a run over 1,024 pages, and chunks were released only when the area was removed
  at close, so their capacity was neither reused by a larger run nor counted against the maximum size. The PRD's
  *Dejado a la implementación* says that memory counts for the maximum size (P10, T14).
- Fix (`7b4286b`): a run gets a chunk of its own, of exactly its size, when it is larger than a normal chunk or when
  the free slots could hold it but no free range is contiguous; that chunk is released (unmapped, and its
  `krummi.temp` file removed) when the run's last holder drops it, so its capacity always belongs to a live version,
  which the maximum-size accounting already counts. Shared chunks grow only when the free slots are fewer than the
  pages to store, as a page-by-page area would; they keep the earlier policy (at most 8 MiB more than the peak of
  held slots, which predates this feature and does not pile up). A transfer pass now frees the versions it retired
  (the existing `free_retired` rule: no open snapshot can hold them), so a transferred run's chunk goes without
  waiting for the next snapshot to end.
- Regression: `crates/krummi-tx/tests/runs.rs::growing_runs_never_pile_up_in_the_temporary_area` (the report's
  scenario: 16 writes of runs of `1025 + i` pages under a 64 MiB maximum, on disk and in temporary mode). RED before
  the fix: `the temporary area holds 8527872 bytes and the store 8429568, with a maximum of 67108864` at write 0, and
  135,462,912 bytes of `krummi.temp` after 16 writes, the report's figure. GREEN after it. Two unit tests in
  `temp.rs`: `a_run_s_own_chunk_is_released_with_it` and `only_a_lack_of_free_slots_grows_the_shared_chunks`.
- Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
  on Windows (372 passed, 0 failed, 13 ignored) and `-p krummi-pager -p krummi-tx -p krummi` on Linux (356 passed);
  the ignored AC-V8 sweeps pass.
- RDD review 5 of `62f7571..3a528ec` approved it (lineage `review-695b76ad34d5b00d`); its advisory findings are in
  the execution document (`odd/tasks/trees-large-values.md`, L16).
- Status: claimed fixed, unverified until a follow-up review.

**Review record**: [2026-10-27-code.md](2026-10-27-code.md), wrapper status `findings`, `threadId`
`01a17f4a-20f3-75d1-a3c4-669c5f80a8fe`, coordinator `gpt-6.1-sol`/`high`, reviewer `krummi-review-critical`
`gpt-6-astra`/`xhigh` (from the wrapper's telemetry).

## 2026-10-27 (verification)

[2026-10-27-code-02.md](2026-10-27-code-02.md), wrapper status `findings`, `threadId`
`01a17f6b-d5fe-7253-84c4-532b728e477f`, coordinator `gpt-6.1-sol`/`high`, reviewer `krummi-review-critical`
`gpt-6-astra`/`xhigh`. Its verdict, quoted: "Changes requested — R1 remains open." Per ID: R1 — "Still open;
original scenario verified fixed, overall finding unresolved." (retained partial runs: a slightly smaller run that
reuses all pages but the last of a freed run left the whole old allocation alive, uncounted, while an old read
blocked the transfer).

## 2026-10-27 (round 2)

Fix commit: `e1cc4ab`.

### R1 — major: retained partial runs still bypass the configured maximum size

- Disposition: fixed.
- Evidence: verified against the code: a run's pages share one `Arc<Slot>` owning the whole allocation, and
  `collect_garbage` subtracted each superseded page version from the scratch byte count while the surviving tail
  entry kept the whole allocation alive; `WriteTransaction::memory` charges that count.
- Fix (`e1cc4ab`): `State::hold` and `State::release` in `crates/krummi-pager/src/scratch.rs` are now the only
  places that change the count. A run is charged its whole `pages × PAGE_SIZE` when its first version entry
  arrives and released when its last entry leaves (`run_holders`, per allocation); publish, garbage collection,
  retirement and close go through them. Single-page versions are counted as before.
- Regression: `crates/krummi-tx/tests/runs.rs::shrinking_runs_under_an_old_read_count_whole_for_the_maximum_size`
  (the follow-up's scenario, on disk and in temporary mode). RED before the fix at the second run: "the temporary
  area holds 17162240 bytes, 8536064 counted, and the store 8552448, with a maximum of 67108864". GREEN: the area
  minus the counted bytes stays at 114,688 bytes (free slots of the first shared chunk), the seventh run fails with
  the maximum-size class and applies nothing, and after the old read closes the count returns to 0 and the area to
  its small size.
- Checks: fmt, clippy, `cargo test --workspace` on Windows (377 passed, 0 failed, 14 ignored), `-p krummi-pager -p
  krummi-tx -p krummi` on Linux (361 passed), and the ignored AC-V8 sweeps pass.
- RDD review 6 of `3a528ec..e1cc4ab` approved it (lineage `review-713a431a84a3734d`). Its advisory finding
  R3-retired-runs-uncounted (WARNING): a run whose versions were retired after their transfer is no longer counted
  while a snapshot open at retirement still holds it. Not changed: retired versions are already in `krummi.data`,
  which the maximum size counts, and retired single pages were never counted either (T14 counts the versions that
  are only in memory); the follow-up review can confirm or reject this reading.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-10-27 (round 3 not run)

The third and last follow-up review (`-FollowUp 2026-10-27-code.md -FixRange 46fdd12..57dc55b`) ended with
wrapper status `error`: Codex usage limit, retry after 2026-10-28 02:35; `threadId`
`01a17f7f-413f-75b1-b21d-21c23a84c773`; no report, no changes. It is not an approval: the round-2 fix of R1
(`e1cc4ab`) stays claimed, unverified by Codex.

## 2026-10-28 (verification, round 3)

Two earlier attempts of the third follow-up produced no report: wrapper status `error` (`threadId`
`01a1812c-3ad2-7e40-8730-a4e8d0bcb7f8`, the CLI's ChatGPT login did not offer `gpt-6.1-sol`) and `error` (`threadId`
`01a18144-178d-72f3-9b74-f3b1fe8a0386`, content flagged as a possible cybersecurity risk while the reviewer wrote a
reproduction under `target/`). The third attempt saved
[2026-10-28-code.md](2026-10-28-code.md): wrapper status `violation`, `unexpectedChanges: ["codex-models.json"]`,
`threadId` `01a18149-b10f-7bc1-9cc0-d8cb8e0ce554`, coordinator `gpt-6.1-sol`/`high`, reviewer
`krummi-review-critical` `gpt-6-astra`/`xhigh`. The unexpected change was the user's: they deleted the untracked
`codex-models.json` (a model catalog they had generated) from the repository root during the run, as they
confirmed on 2026-10-28. Its verdict, quoted: "Changes requested — R1 remains open." Per ID: R1 — "Still open;
both earlier scenarios verified fixed, retirement accounting unresolved." The round-2 rationale for leaving
`R3-retired-runs-uncounted` unchanged was rebutted with a two-reader reproduction (93,396,992 bytes of temporary
area under a 67,108,864-byte maximum): overlapping, shrinking runs make a retired allocation extra capacity, not a
copy of `krummi.data`. The user authorized a fourth round beyond the three-round limit on 2026-10-28.

## 2026-10-28 (round 3)

Fix commit: `098e621`.

### R1 — major: retired contiguous run allocations still bypass the configured maximum size

- Disposition: fixed.
- Evidence: verified against the code: `State::retire` released a run's whole charge when its last entry left
  `versions`, while the allocation stayed alive in `retired` until `free_retired` dropped it; the round-3 rebuttal
  holds, because overlapping, shrinking runs make that allocation extra capacity rather than a copy of
  `krummi.data`.
- Fix (`098e621`): `run_holders` counts a run's entries in both `versions` and `retired`; retiring a run version
  moves its share of the charge without changing the count, and `free_retired` releases it, so the whole charge goes
  only when the run's last holder in either place goes, just before the allocation is dropped. Retired single pages
  stay uncounted (`counted_while_retired`): they copy pages of `krummi.data`. Close frees retired versions with
  `free_retired` before its zero-count debug assertion. Consequence near the limit: runs that a write's own flush
  retires stay counted until that write ends, so that write can now fail with the maximum-size class; the next one
  gets the room back.
- Regression: `crates/krummi-tx/tests/runs.rs::shrinking_runs_retired_under_a_newer_read_count_until_freed` (the
  round-3 two-reader scenario, on disk and in temporary mode). RED before the fix: at "the old read ended", area
  51,126,272 bytes, counted 0, store 8,552,448, maximum 67,108,864 (a probe matched the review's figures up to
  93,396,992 / 42,287,104 at 1,030 pages). GREEN: counted 50,995,200 against the same area after the transfer, every
  replacement from 1,034 to 1,030 pages fails with the maximum-size class and applies nothing, the reference keeps
  its bytes, and the area empties after the second read ends.
- Checks: fmt, clippy, `cargo test --workspace` on Windows (378 passed, 0 failed, 14 ignored), `-p krummi-pager -p
  krummi-tx -p krummi` on Linux (362 passed), and the ignored AC-V8 sweeps pass.
- RDD review 7 of `373208f..098e621` approved it (lineage `review-aba90cc61b80a4bb`). R3-001 (WARNING, close might
  keep retired runs and trip the zero-count assertion): dismissed with evidence: with no snapshot and no writer open
  `free_retired` takes `oldest = u64::MAX` (`crates/krummi-pager/src/scratch.rs:879-886`), so every retired entry is
  freed at close. R3-002 (SUGGESTION, a direct assertion on the count while the newer read is open): accepted without
  change; the test already requires every replacement to fail with the maximum-size class.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-10-28 (verification, round 4)

[2026-10-28-code-02.md](2026-10-28-code-02.md), wrapper status `findings`, `threadId`
`01a18164-ea2a-7a81-b2f0-cc836a1af0c5`, coordinator `gpt-6.1-sol`/`high`, reviewer `krummi-review-critical`
`gpt-6-astra`/`xhigh`. Its verdict, quoted: "Changes requested — R1 verified resolved; R2 remains open." R1 is
verified resolved. New R2 (major): "A writer's own flush causes a maximum-size failure without readers", introduced
by `098e621`, against J7 ("una escritura nunca falla por tamaño máximo a causa de versiones que ninguna lectura
necesita: antes se trasladan").

## 2026-10-28 (round 4)

Fix commit: `7584790`.

### R2 — major: a writer's own flush causes a maximum-size failure without readers

- Disposition: fixed.
- Evidence: verified against the code: `WriteTransaction::reserve` flushes and re-checks the budget, but
  `free_retired` kept every retired entry whose bound was not below `min(oldest read, writer seq)`, so a run retired
  by the writer's own flush stayed charged until the write ended.
- Fix (`7584790`): `ScratchWriter::flush(&mut self)` now frees the retired entries no open read can reference
  (`free_retired_by_writer`, bound `oldest_read()`), ignoring the writer's own sequence; the transfer thread, snapshot
  end and close keep the earlier rule. It is safe because every reference the writer's snapshot returns borrows the
  writer: `ScratchWriter::snapshot(&self) -> &Snapshot<'a>` (`crates/krummi-pager/src/scratch.rs:1069`), and
  `Snapshot::page`/`Snapshot::run` take `&self` and return a slice of that borrow (`:950`, `:989`), so `flush(&mut
  self)` cannot run while one is alive (T8); the compiler enforces it for every internal caller.
- Regression: `crates/krummi-tx/tests/runs.rs::a_write_s_own_transfer_frees_the_runs_no_read_needs` (the report's
  scenario with a 24 MiB maximum, no read, and with a read that ended before the third write; disk and temporary).
  RED: `MaxSize { max_size: 25165824, needed: 25608192 }` at the third write's reservation. GREEN: the same first
  page, the commit succeeds, the data size is unchanged.
- Checks: fmt, clippy, `cargo test --workspace` on Windows (379 passed, 0 failed, 14 ignored), `-p krummi-pager -p
  krummi-tx -p krummi` on Linux (363 passed), the ignored AC-V8 sweeps and `ac_j18_…` pass.
- RDD review 8 of `45c1bf2..7584790` approved it (lineage `review-48af2c09d934d928`). R1-writer-flush-frees-under-
  unsafe-invariant and R3-writer-flush-lifetime-assumption (WARNING, the safety depends on writer references
  borrowing the writer): dismissed with evidence, the signatures above. R3-missing-open-read-boundary-test
  (SUGGESTION): dismissed with evidence: `shrinking_runs_retired_under_a_newer_read_count_until_freed` keeps a read
  open across the flushes of every replacement and checks its reference keeps its bytes.
- Status: claimed fixed, unverified until a follow-up review.
