# Codex review automation code review resolutions

Resolutions of [2026-10-26-code.md](2026-10-26-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-10-26

Review record: report `2026-10-26-code.md`, wrapper status `findings`, thread `01a1772f-da2c-7f32-85a4-c1e0777075bc`.
Executed settings from the session files: coordinator `gpt-6.1-sol` / `high`; one `krummi-review` reviewer
`gpt-6.1-sol` / `high`.

Fix commits on `feat/codex-review`: `c47b0be` (R1), `7afccd2` (R2), `e670fb4` (observation below).
Checks after the fixes: Pester 17 passed, 0 failed.

### R1 — major: Follow-up rejects numbered original reports

- Disposition: fixed (`c47b0be`).
- Fix: a follow-up accepts any review report name, `<date>-<prd|code>[-NN].md`. What identifies the original of a
  chain is its own `-resolutions.md`: follow-up reports share the original's, so passing one still stops with
  "No resolutions to verify", which now says to pass the chain's original report. New test: a second chain's
  `<date>-prd-02.md` with its resolutions runs; the refusal test now expects the resolutions message.
- RED observed before the fix: the new test failed with status `error`.
- Status: claimed fixed, unverified until a follow-up review.

### R2 — major: Failed process termination defeats the review timeout

- Disposition: fixed (`7afccd2`).
- Fix: on timeout the wrapper stops the process tree with `Process.Kill($true)` instead of `taskkill`, and every
  wait after it is bounded (10 s for the exit, 30 s for the output streams). When the process cannot be stopped,
  the result is still `error`, naming the process ID and warning that it may still change files. The timeout
  test now also requires the wrapper to return in under 20 s while the fake hangs for 30 s.
- RED not reproducible here: outside Codex's sandbox `taskkill` succeeds, so the timing test passed before the
  fix too (5.5 s). The fix follows the report's own reproduction, in which `.Kill($true)` stopped a child that
  `taskkill` could not.
- Status: claimed fixed, unverified until a follow-up review.

### Observation in *Checks and limitations*: `approved` given as a string

Not a finding of the report. The wrapper accepted `"approved": "false"` as an approval. Since that contradicts S3
("Un error de ejecución de Codex no equivale a una aprobación"), `e670fb4` treats a non-boolean `approved` as an
invalid result (`error`). New test; RED observed before the fix (status `approved`).

## 2026-10-26 (verification)

Review record: follow-up report [2026-10-26-code-02.md](2026-10-26-code-02.md), wrapper status `findings`, thread
`01a1773a-2e99-7431-a089-058a89b4aca3`. Executed settings from the session files: coordinator `gpt-6.1-sol` /
`high`; one `krummi-review` reviewer `gpt-6.1-sol` / `high`.

- Verdict: "Changes requested — R1 remains open (minor); R2 verified resolved."
- R2: verified resolved. The boolean `approved` hardening was checked for regressions; none found.
- R1: "partially corrected, still open", severity reduced to minor: the allocator writes `-100` and above
  (`D2` is a minimum width), and the follow-up accepted only two digits.

## 2026-10-26 (round 2)

### R1 — minor: Follow-up still rejects originals numbered 100 and above

- Disposition: fixed (`2499228`).
- Fix: the follow-up accepts any numeric suffix of two or more digits, `(-\d{2,})?`, which covers every name
  the allocator produces. New test with `<date>-prd-100.md` and its resolutions; RED observed before the fix
  (status `error`). Pester 18 passed, 0 failed.
- Status: claimed fixed, unverified until a follow-up review.
