# CI fixes code review resolutions

Resolutions of [2026-11-19-code.md](2026-11-19-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-19

Review record: [2026-11-19-code.md](2026-11-19-code.md), range `e5b5f0b..86d9ccd`, wrapper status `findings`,
verdict «Changes requested: R1 remains open (minor documentation issue).» `threadId`
`01a1f2ef-ad76-7060-8317-c1bd67a80914`; telemetry: coordinator `gpt-6.1-sol`/`high`, reviewer `krummi-review`
(Goodall) `gpt-6.1-sol`/`high`. An earlier attempt ended in `error` («Codex CLI not found: codex») after a Node
version change left the CLI in the previous version's global packages; it was reinstalled (0.170.0) and the review
rerun.

### R1 — minor: the README promises an independent runner per sweep, but the four share one job

- Disposition: fixed (`34b54a0`).
- Fix: the README now says the full acceptance sweeps run «in a separate job per system, apart from the checks, so
  the sweeps have their own runner and time budget».
- Status: claimed fixed, unverified until a follow-up review.
