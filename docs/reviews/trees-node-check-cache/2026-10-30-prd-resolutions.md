# Node check cache PRD review resolutions

Resolutions of [2026-10-30-prd.md](2026-10-30-prd.md). Append-only: each update is dated and keeps the earlier ones.
A claimed fix stays unverified until a follow-up review confirms it.

## 2026-10-30

Artifact version: uncommitted `docs/prd/krummi/trees-node-check-cache.prd.md`, SHA-256 `20d3582bba08455e6d73b6c44c2270788532b3a0f3e2696dcf6919662108046f`. The annotations in
`krummi.prd.md`, `trees.prd.md`, `trees-keys.prd.md` and `trees-multivalue.prd.md` are unchanged since the review.

### R1 — minor: C7 overstates the memory-safety guarantee for external file mutation

- Disposition: fixed.
- Evidence: verified against the ancestry. The root's D12 annotation and P16 keep undefined behaviour possible when
  another program modifies the store's files while it is open, and C7 listed that scenario before promising no
  undefined behaviour.
- Fix: C7 now says that using a malformed node does not by itself cause undefined behaviour (the trees use no unsafe
  code, D12), and that the P16 exception stays as it is: a foreign program modifying the files of an open store may
  cause undefined behaviour, as today.
- Status: claimed fixed, unverified until a follow-up review.

Review record: [2026-10-30-prd.md](2026-10-30-prd.md), wrapper status `violation` with unexpected changes `HEAD`,
`refs` and `docs/prd/documents/documents.prd.md`. They were not Codex's: during the review the user committed their
own draft `documents.prd.md` on the branch (`308e3a0`, 08:40), and Claude fast-forwarded `main` to that commit and
pushed it at the user's request. The report records the same external commit and that the hashes of every reviewed
file and of the reviewed diff did not change. Nothing was reverted. Thread `01a18bf6-8790-72a3-97cb-67ecef0ca532`,
coordinator `gpt-6.1-sol`/`high`, reviewer `krummi-review-critical` `gpt-6-astra`/`xhigh` (telemetry).

## 2026-10-30 (verification)

Follow-up report: [2026-10-30-prd-02.md](2026-10-30-prd-02.md). Verdict, quoted: "Ready for implementation within the
requested follow-up scope. R1 is verified resolved; no open findings or regressions caused by its correction were
identified."

- R1: verified resolved.

Review record: wrapper status `approved`, thread `01a18bff-491a-7b32-bf4f-421cb1f83df4`, coordinator
`gpt-6.1-sol`/`high`, reviewer `krummi-review-critical` `gpt-6-astra`/`xhigh` (telemetry).
