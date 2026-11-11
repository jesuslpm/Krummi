# Tables JSON design review resolutions (post-approval range)

Resolutions of [2026-11-11-code-03.md](2026-11-11-code-03.md), the review of `3bbd2da..99277d1` (the commits after
the approved chain of [2026-11-11-code.md](2026-11-11-code.md)). Append-only: each update is dated and keeps the
earlier ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-11

Reviewed range: `3bbd2da..99277d1`. Fix commit: `e352bca` (R1).

Review record: `docs/reviews/tables-json/2026-11-11-code-03.md`, wrapper status `findings` (open R1), `threadId`
`01a1ca40-278e-74d1-9384-cade7f88a53c`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. The eight open product questions the report lists are the ones
document 14 defers until the indexes are planned; none is new, and none is answered here.

### R1 — minor: Dictionary prefix scans omit hashed string terms

- Disposition: fixed.
- Verification before the fix: *Prefixes and ranges over the dictionary* said that every value term of a path starts
  with `V ‖ path ‖ kind` and that string prefixes and ranges can use the dictionary as designed, while *Terms* stores
  a long term as `H ‖ xxh3-64` with no such prefix; a scan of the `V` prefix cannot reach it. The report's model shows
  it (`full scan [1, 2] V-prefix scan [2]`). Rule 1, which still gives ranges no list, is unaffected.
- Fix: a new item in that section says the scans are complete only over unhashed terms and records the two ways to
  enable them in the detailed design: hashed terms that keep their path and the first bytes of their value before the
  hash, scanned and rechecked; or no list and a full scan for paths that may hold hashed values. A path whose own
  encoding is too long always falls back.
- Status: claimed fixed, unverified until a follow-up review.

Checks for the follow-up: `git diff --check 99277d1..e352bca`, no output. The range changes one Markdown document;
the cargo checks of `CLAUDE.md` do not apply to it.

## 2026-11-11 (verification)

Follow-up report: [2026-11-11-code-04.md](2026-11-11-code-04.md), wrapper status `approved`, `threadId`
`01a1ca46-54ea-7860-8988-5b1a88cf0e30`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`. Fix range `99277d1..5caac57`. Verdict, quoted: "Approved — R1
verified resolved; no findings remain open in the requested follow-up scope."

- R1: "Verified resolved".

No product question was added; the eight deferred questions of document 14 stay open until the indexes are planned.
