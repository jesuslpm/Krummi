# Decisions of 2026-11-06 review resolutions

Resolutions of [2026-11-06-code.md](2026-11-06-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-06

### R1 — minor: AC-I12 overstates the coverage of the individual measurements

- Disposition: fixed.
- Fix: the missing Linux comparisons were measured against `3ec8641` (pre-O3) and the O4 head build, with the
  binaries and method of L10 of `odd/tasks/commit-copies.md` (`work/o4/full.sh`, two rounds, order swapped).
  Ratios head / pre per round:
  - W4: 0.95, 1.03.
  - W5: 1.00, 1.02.
  - W6: 0.94, 0.99, then 1.03 and 1.06 in two more rounds.
- The annotation to AC-I12 now lists what was measured and how, in both languages, instead of a universal claim.
- Evidence: `work/o4/full-wsl-w456.out`, `~/o4full/work/pre.tsv` and `head.tsv` in WSL (not in git).
- Status: claimed, unverified until a follow-up review.

### R2 — minor: The 64 TiB annotation omits stores with a larger configured maximum

- Disposition: fixed.
- Fix: the annotation to P10 says the 64 TiB ceiling holds both without a maximum size and with one larger than
  64 TiB, in both languages.
- Status: claimed, unverified until a follow-up review.

Review record: [2026-11-06-code.md](2026-11-06-code.md), wrapper status `findings`, threadId
`01a1b273-4e55-7781-ad9d-00340e88259f`. The telemetry reports the requested settings: coordinator `gpt-6.1-sol` /
`high`.
