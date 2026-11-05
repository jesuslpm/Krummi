# Remembered page integrity PRD review resolutions

Resolutions of [2026-11-05-prd.md](2026-11-05-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-05

Artifact versions after the fixes: commit `7050669`
(`docs/prd/krummi/pager-integrity-once.prd.md`, `docs/prd/krummi/trees-large-values.prd.md`,
`docs/prd/krummi/trees.prd.md`).

### R1 — minor: Missing annotations for superseded tree integrity rules

- Disposition: fixed.
- Evidence of the defect: `trees-large-values.prd.md:47` (V7, «vuelven a fallar con esa clase cada vez que lo
  necesitan») and `:155`–`:158` (checking «en cada lectura o una vez por transacción» left to the implementation),
  and `trees.prd.md:107` («sigue en cada lectura del archivo de datos»), carried no annotation from this PRD.
- Fix: `trees-large-values.prd.md` gets a dated 2026-11-05 annotation block for V7 (a damaged value keeps failing
  because a failed check is never remembered, I1; the guarantee carries the residual risk of I8) and for *Dejado a
  la implementación* (the per-read or per-transaction choice is no longer open: I1 to I4 fix it). `trees.prd.md`
  gets a dated 2026-11-05 annotation for *Reutilización* (integrity follows I2 and I3, with the residual risk of
  I8 replacing C7's). `pager-integrity-once.prd.md` lists both in *Anotaciones a otros PRD*.
- Status: claimed fixed, unverified until a follow-up review.

Review record: [2026-11-05-prd.md](2026-11-05-prd.md), wrapper status `findings` (R1 open), thread
`01a1ad0b-e2df-7572-80f3-664146eeaf34`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

## 2026-11-05 (verification)

Follow-up [2026-11-05-prd-02.md](2026-11-05-prd-02.md) over `21a8d11..d7869a6`, wrapper status `approved`, thread
`01a1ad14-090b-75d0-be03-acda7cff6530`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer `krummi-review`
`gpt-6.1-sol` / `high`.

- R1: «verified resolved». Verdict: «Ready for implementation within the requested follow-up scope.»
