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

## 2026-11-17 (update)

Artifact versions: the fixes in `f45269d` and `cf8fd80`; the matrix and the RDD record in the execution document,
`odd/tasks/write-optimizations.md` L11–L13. Fix range for the follow-up: `2417fd9..HEAD`.

### R2 — major: Q3 metadata adds an allocation that can fail during commit

Disposition: fixed in `cf8fd80`. `ScratchWriter::commit` takes one `PageChange` per page (`Created`, `Copied`,
`KeepsCheck`) in place of the created flags, so the keep-check mark travels in the slice the commit already
builds; no new `Vec` is reserved for it (`crates/krummi-pager/src/scratch.rs`, `crates/krummi-tx/src/transaction.rs`).
Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test
--workspace` 867 passed, 0 failed, 35 ignored. RDD of `e0e4fbe..5bbeb4e` (lenses risk, resilience, reliability,
lineage `review-56d68a6bfbe776dd`): approved, two non-blocking advisories recorded in L12.

Status: claimed, unverified until a follow-up review.

### R3 — major: the required performance acceptance matrix is incomplete

Disposition: fixed: the Q7 A/B on the final code (`cf8fd80`) against the write-margin head (`10077aa`), every load
Q7 names, both platforms, ≥3 rounds and ≥10 for the short or multithreaded loads.

- Windows (L11): W28 1.075, W27 1.053, W12 1.028, W3 1.025, W17 1.022, W7 1.021, W26 1.020, W22 1.016, the rest
  0.995–1.005; W21 1.001 and W23 0.998 (slower in 8 of 10) in 10 rounds. W23 is a worsening of 0.2 % by the letter of
  Q7, under one run's spread; Q7 leaves it to the user, and the closing report lists it.
- WSL 2 (L13): W27 1.166, W23 1.085, W3 1.071, W28 1.071, the rest about 1.0; the loads that came out slower (W5, W12,
  W14, W15, W21, W24, W25) measured again in 20 or 6 rounds, each slower in about half its rounds (medians
  0.993–1.065): none is a worsening.

Status: claimed, unverified until a follow-up review.

### R4 — minor: debug cache lookups add blocking synchronization between readers

Disposition: fixed in `cf8fd80`. A debug-only array of `AtomicU64` beside the checked table (`Checked::kept`, one
entry per slot) replaces the mutex map; `Checked::layout_kept` is one relaxed load, with no lock
(`crates/krummi-pager/src/checked.rs`). Checks as in R2.

Status: claimed, unverified until a follow-up review.
