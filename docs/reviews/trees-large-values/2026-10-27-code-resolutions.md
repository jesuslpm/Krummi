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
