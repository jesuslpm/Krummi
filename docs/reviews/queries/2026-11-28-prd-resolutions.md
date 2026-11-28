# Queries PRD review resolutions

Resolutions of [2026-11-28-prd.md](2026-11-28-prd.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-28

Review record: [2026-11-28-prd.md](2026-11-28-prd.md), wrapper status `findings` (R1 open), `threadId`
`01a221b9-a7d3-73f3-a24a-c48fbbc2b1a0`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` (Euclid) `gpt-6-astra` / `xhigh`.

Artifact versions after the fixes (uncommitted working tree, SHA-256 of the bytes):

- `docs/prd/krummi/queries/queries.prd.md`: `c3cf850baaa19202ce1dab621668974abe83f6d9bfa0689986729d12d39aa42f`

### R1 — major: Unqualified statement recovery conflicts with transaction-fatal storage errors

- Disposition: fixed.
- Fix: the 17.7 row now limits statement-local recovery to public validation errors as B7 defines them (invalid
  value, duplicate key, invalid size): such a statement leaves none of its changes and the transaction can still
  commit the others. A failure that T11 makes fatal for the transaction (disk full, maximum size, out of memory,
  damage, I/O) stays fatal, and T12 still applies to the store. No guarantee of the ancestry is overridden, so the
  report's open product question does not arise. Whether a write statement's own failures (a CS8 budget exceeded, a
  cancellation) keep the transaction committable is delegated to 17.7 in its *Fija* cell. *Referencias* now cite T11
  and T12.
- Status: claimed fixed, unverified until a follow-up review.

## 2026-11-28 (verification)

Review record: [2026-11-28-prd-02.md](2026-11-28-prd-02.md), wrapper status `approved`, `threadId`
`01a221bf-a3bb-7a30-a0f3-fc054069c166`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` (Singer) `gpt-6-astra` / `xhigh`.

- R1: "Disposition: verified resolved."

Verdict, quoted: "Ready for implementation within the reviewed follow-up scope. R1 is verified resolved; no findings
remain open. This umbrella review does not authorize implementation or replace the required child PRDs."
