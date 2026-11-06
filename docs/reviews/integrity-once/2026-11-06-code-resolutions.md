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

## 2026-11-06 (verification)

Follow-up [2026-11-06-code-02.md](2026-11-06-code-02.md) over `362674a..7c9c6f3`, wrapper status `findings`,
thread `01a1b00c-4761-77f3-aaf8-a1d38d9209e9`: R1 and R2 «verified resolved»; R3 (major) and R4 (minor) opened
against `e5ea536`; the 64 TiB question still open. Verdict: «Changes requested: R3 (major) and R4 (minor) remain
open; R1 and R2 are verified resolved.»

## 2026-11-06

Fix commit: `c29ac23`.

### R3 — major: Shared table eviction makes unchanged values repeat integrity checks

- Disposition: fixed.
- Fix: a run is remembered by the bits of all its pages again, and the mark in the table of checked contents only
  adds a check: its slot keeps the run's length (`Checked::remember_run`, `run_length`; high bits from the stored
  code changed by a constant, so a node's code never matches). While the mark is there, a read of another length is
  checked (R1 stays fixed); when another content takes the slot, the run stays remembered by its bits.
- Evidence: `integrity_tests::ac_i1_a_value_stays_remembered_when_its_mark_is_replaced` (one slot, so every read
  of the value also takes the slot for its leaf's layout): after the first read, three more reads of the value make
  0 integrity checks, on disk and in a temporary store. Under `e5ea536` this is the reported repetition.
- Residual, recorded: once the mark's slot is taken, a read of the wrong length from that first page (only a bad
  reference) is not detected until the run is written again or the store reopens.
- Status: claimed fixed, unverified until a follow-up review.

### R4 — minor: Rewriting only a run's head does not reliably invalidate it

- Disposition: fixed.
- Fix: the first page's bit is part of the run again, so any write of it forgets the run, also with the same bytes.
- Evidence: `pager::tests::a_remembered_run_does_not_vouch_for_another_extent` rewrites the first page alone with
  its own bytes: the run is no longer remembered, and the next read checks it once and passes.
- Status: claimed fixed, unverified until a follow-up review.

Checks after the fix: `cargo fmt --all --check` ok, `cargo clippy --workspace --all-targets -- -D warnings`
clean, `cargo test --workspace` 542 passed, 0 failed, 30 ignored.

## 2026-11-06 (verification)

A first run of the third round ended with wrapper status `error`, before any review: the provider flagged the request
as a possible cybersecurity risk (thread `01a1b020-f97d-7671-8e05-4837f7212a26`, no report). It was rerun once
unchanged. Follow-up [2026-11-06-code-03.md](2026-11-06-code-03.md) over `7c9c6f3..5a6479b`, wrapper status
`findings`, thread `01a1b024-d3ff-7171-99ab-e4be239a758a`: R2 remains resolved, R3 and R4 «verified resolved», R1
reopened. Verdict: «Changes requested: R1 (major) is reopened; R3 and R4 are verified resolved, and R2 remains
resolved.» The 64 TiB question stays open.

### R1 — major (reopened): Run extent validation disappears when its length mark is replaced

- Disposition: pending, a product decision for the user. One bit per page (I6) cannot hold a run's length, so an
  exact extent for every remembered run needs memory beyond I6, or a mark that can be replaced. Keeping the mark
  required (`e5ea536`) made unchanged values checked on every read once their mark was replaced, against AC-I1 (R3);
  making it optional (`c29ac23`) leaves a read of the wrong length from a run's first page unchecked once the mark is
  replaced, which only a bad reference makes. The three-round limit is reached; the finding stays open.
- Status: open.
