# Remembered page integrity code review resolutions

Resolutions of [2026-11-06-code.md](2026-11-06-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-06

Fix commit: `e5ea536`.

### R1 — major: Remembered bits do not preserve the verified run's exact extent

- Disposition: fixed.
- Fix: a run of several pages is remembered by the bits of its interior pages and, for its first page, a mark of
  its stored code mixed with its page count, kept in the existing table of checked contents (`checked.rs`,
  `remember_run`, not counted as a checked node); never by its first page's bit. A read of the first page alone, a
  shorter or longer run from it, or a run from an interior page finds no matching mark or bit and is checked, as at
  the base; any write of an interior page clears its bit, and a rewritten first page has another stored code. The
  table is fixed memory that the store already has, so I6's bound on the remembered pages holds.
- Evidence: `pager::tests::a_remembered_run_does_not_vouch_for_another_extent` reproduces the reported scenario at
  pager level (a three-page run remembered, then 1 and 2 pages from its first page, and its first page as a page):
  each read fails with the damage class under every policy, and the run stays remembered.
  `krummi` `integrity_tests` now assert the value run with its length (`is_run_remembered`).
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: A zero-page remembered read panics before validation

- Disposition: fixed.
- Fix: the fast path reads the page number with `get(8..16)` and remembers nothing for zero pages, so a read of no
  pages reaches `verify_run`, which rejects it as damage.
- Evidence: the same test reads 0 pages under `Check`, `CheckAndRemember` and `UseRemembered`: damage each time.
- Status: claimed fixed, unverified until a follow-up review.

### Open product question — the 64 TiB growth ceiling

- Disposition: pending, for the user after the loop.

Checks after the fix: `cargo fmt --all --check` ok, `cargo clippy --workspace --all-targets -- -D warnings`
clean, `cargo test --workspace` 541 passed, 0 failed, 30 ignored.

Review record: [2026-11-06-code.md](2026-11-06-code.md), wrapper status `findings` (R1, R2 open, one product
question), thread `01a1affb-269a-78c3-9b6e-9aed79a5c18f`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.
