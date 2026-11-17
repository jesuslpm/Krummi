# Write optimizations code review resolutions

Resolutions of [2026-11-17-code.md](2026-11-17-code.md). Append-only: each update is dated and keeps the earlier
ones. A claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-17

Artifact versions: reviewed range `10077aa..2417fd9`; the fixes in the commits after `35f3b5d`.

### R1 — major: sampling skips compression of strongly compressible records

Disposition: fixed by removing the change. The finding holds: a sample of 32 KiB cannot see a repetition longer than
itself, so no sampling rule keeps Y1 («se comprime como hoy cuando ocupa menos comprimido») for every record, and the
journal growth the review reproduced (4,267,120 bytes stored raw for a record that compresses to 135,568) is the kind
of cost J16 rules out. T2 was reverted in `f45269d`: the journal compresses every commit record whole again, as at
`10077aa` (`git diff 10077aa f45269d -- crates/krummi-pager/src/journal.rs` is empty). W17's gain of about 4 % is
given up; a rule that is exact (one that only skips compression when it cannot help) is left for a later change.

Status: claimed, unverified until a follow-up review.

### R2 — major: Q3 metadata adds an allocation that can fail during commit

Disposition: being fixed (writer in progress): the keep-check flags travel in memory the commit already reserves.

Status: pending this update.

### R3 — major: the required performance acceptance matrix is incomplete

Disposition: being fixed: the A/B of Q7 on the final code, every load it names, Windows and WSL, with the rounds it
asks for, after R2 and R4.

Status: pending this update.

### R4 — minor: debug cache lookups add blocking synchronization between readers

Disposition: being fixed (writer in progress): a lock-free, fixed-size, debug-only record replaces the mutex.

Status: pending this update.

Review record: [2026-11-17-code.md](2026-11-17-code.md), wrapper status `findings` (R1–R4 open; no open question),
thread `01a1e74e-39be-7ef0-a0de-7880a2b86b9e`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` (Plato) `gpt-6-astra` / `xhigh`.
