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
