# Max-size growth code review resolutions

Resolutions of [2026-10-27-code.md](2026-10-27-code.md), the original report of this chain. Append-only: each
update is dated and keeps the earlier ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-10-27

Review record: [2026-10-27-code.md](2026-10-27-code.md), wrapper status `findings`, `threadId`
`01a17c18-990f-7952-afcf-af6aa2cf6da5`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. Range reviewed: `990689b..e341f00`. Fix commits: `ce4f264`,
`a7937d5` (branch `fix/max-size-growth-transfer-wait`).

### R1 — major: Growth waits for transfer even when the maximum leaves ample room

- Disposition: fixed in `ce4f264`.
- Fix: `WriteTransaction::reserve` (`crates/krummi-tx/src/transaction.rs`) transfers before a growth only when the
  waiting versions would cut it: when the pager's growth target capped at the maximum, plus the scratch area and the
  transaction's changes, exceeds the maximum. The target comes from `growth_target`
  (`crates/krummi-pager/src/pager.rs`), extracted from the pager's own sizing so both use the same rule. Otherwise
  the store takes its full growth without waiting for the transfer (J5, AC-J17); when the versions would cut it,
  the earlier behavior stays, so the size still does not depend on when the transfer ran (J7).
- Evidence: `crates/krummi-tx/tests/free_space.rs`
  `growth_with_ample_room_does_not_wait_for_a_running_transfer` reproduces the report's scenario (64 MiB maximum,
  versions waiting, a transfer held by its hook, a write that grows the store): RED at `e341f00`, GREEN at
  `ce4f264`; its wait was raised from 5 s to 30 s in `a7937d5` for slow machines.
  `growth_within_the_maximum_size_ignores_versions_no_read_needs` and
  `growth_within_the_maximum_size_counts_versions_a_read_needs` still pass. `cargo test --workspace`: 328 pass,
  0 fail, 11 ignored; fmt and clippy clean; the `maximum_size` tests in four parallel processes, five runs each:
  20 of 20 pass.
- Status: claimed fixed, unverified until a follow-up review.
