# Krummi and Voron only benchmark PRD review resolutions

Resolutions of [2026-11-12-prd.md](2026-11-12-prd.md). Append-only: each update is dated and keeps the earlier ones. A
claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-12

Artifact versions: `docs/prd/krummi/benchmark/benchmark-krummi-voron.prd.md` uncommitted, SHA-256 starting `8d00cc9199f8fcb3`.

### R1 — major: AC-B3 requires unchanged results when affinity fallback can change them

- Disposition: fixed.
- Evidence: the finding holds: measuring without limits can change the rates, the thresholds and the exit code.
- Fix: AC-B3's failure case now injects the same results by load and engine and the same machine data into the run
  with the failure and the run without it, and states that in a real run the thresholds and the exit code still come
  from what was measured (K7, K13).
- Status: claimed fixed, unverified until a follow-up review.

Review record: [2026-11-12-prd.md](2026-11-12-prd.md), wrapper status `findings`, thread
`01a1d00b-c11c-7a33-a3be-d67785ddcf3e`.

## 2026-11-12 (verification)

Follow-up report: [2026-11-12-prd-02.md](2026-11-12-prd-02.md), thread `01a1d010-b927-7261-9ed9-1f15cbf7ba04`,
wrapper status `approved`, verdict "Ready for implementation within the reviewed follow-up scope."
