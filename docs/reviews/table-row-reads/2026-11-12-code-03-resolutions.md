# Table row reads code review resolutions (third round: insert path)

Resolutions of [2026-11-12-code-03.md](2026-11-12-code-03.md), the review of `18e38f7..29d0694` (T9–T11 of the row
reads run). Append-only: each update is dated and keeps the earlier ones. A claimed fix stays unverified until a
follow-up review confirms it.

## 2026-11-12

Artifact versions: fix in commit `ce11e62`, on branch `feat/table-row-reads`.

### R1 — minor: Beside::Unknown documents a valid tree state as impossible

- Disposition: fixed.
- Evidence: the finding holds: `drop_child` can leave a non-root branch with one child and no separator when it cannot
  merge with its sibling (the state the existing test at `crates/krummi-tree/src/tests.rs:655` reaches).
- Fix (`ce11e62`): the comment of `Beside::Unknown` describes that state as valid and says why an underfull leaf beside it
  takes the regular walk (`merge_child` passes it up to the ancestors, which may change). No code change.
- Status: claimed fixed, unverified until a follow-up review.

Checks after the fix: `cargo fmt --all --check`; `cargo clippy -p krummi-tree --all-targets -- -D warnings` clean.

Review record: [2026-11-12-code-03.md](2026-11-12-code-03.md), wrapper status `findings`, thread
`01a1d1b3-e8dd-7bb0-bf8d-7831c209e632`.

## 2026-11-12 (verification)

Follow-up report: [2026-11-12-code-04.md](2026-11-12-code-04.md), fix range `29d0694..e4e19de`, thread
`01a1d1bf-f80f-7861-803f-5746cf093eff`, wrapper status `approved`.

## 2026-11-12 (verification, round 3)

Follow-up over `e4e19de..abf5555` ([report](2026-11-12-code-05.md)): «Approved for the requested follow-up scope: R1
remains verified resolved; no open findings.» The report says the default-borrow test change (`d92a88f`) was not
reopened, so this chain gives no Codex verdict on it; its review is the RDD of the last slice
(`review-8123cb28685cc534`, ODD L52). The round limit of the chain is reached.

Review record: [2026-11-12-code-05.md](2026-11-12-code-05.md), wrapper status `violation`, thread
`01a1d227-f45c-7803-95a2-384fd8c4c7a1`, coordinator `gpt-6.1-sol` high, reviewer `krummi-review` `gpt-6.1-sol` high
(telemetry). The violation names only `odd/done/tables/table-row-reads-report.md`, which Claude edited while the review ran
(the RDD row of the last slice and the cleanup section); Codex's own changes were only its report.
