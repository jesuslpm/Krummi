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

## 2026-11-17 (verification)

Follow-up report: [2026-11-17-code-02.md](2026-11-17-code-02.md), fix range `2417fd9..df8abb1`, verdict «Changes
required.» Per ID, quoted: R1 «Verified resolved.»; R2 «Verified resolved.»; R3 «Partially resolved; still open as R3,
reduced to minor.»; R4 «Verified resolved for the original blocking defect.», with the new R5 (major) introduced by its
fix.

Review record: [2026-11-17-code-02.md](2026-11-17-code-02.md), wrapper status `findings` (R5 major, R3 minor open; no
open question), thread `01a1e8ad-e6d2-7de0-a313-2e5992c6af6e`. Telemetry: coordinator `gpt-6.1-sol` / `high`;
reviewer `krummi-review-critical` (Galileo) `gpt-6-astra` / `xhigh`.

## 2026-11-17 (round 2 resolutions)

Artifact versions: the fix in `b025d58`; the corrected figures in the execution document, L14.

### R5 — major: colliding transfer can suppress Q4's debug validation

Disposition: fixed in `b025d58`. The finding holds: a read finds the content in its slot, the transfer writes another
page of the same slot, and the read then asks for a record that now names the other page. `Checked::layout_kept` now
answers yes when the slot no longer holds the content the read found, as well as when its record names it: the read
checks a layout that may have kept its check. The transfer stores the slot before the record, releasing it, and the read
acquires the record before it looks at the slot, so a read that sees a later record also sees the slot change before it.
Only debug builds change: in release builds `remember_written` still stores the slot alone, as before. No lock (C6).

Fix: test `checked::tests::a_kept_content_replaced_after_its_hit_is_still_checked` (RED before the fix, at its
`layout_kept(1, 10)` after `remember_written(5, 50, false)`, GREEN after), the interleaving of the report's probes
with the lookups in their order; `a_kept_check_is_recorded_for_its_content_only` keeps its cases for the contents the
slot holds. Checks: `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo
test --workspace` 868 passed, 0 failed, 35 ignored; `cargo test --release -p krummi --lib written_pages_tests` 7
passed.

Status: claimed, unverified until a follow-up review.

### R3 — minor: Q7 assessment omits two measured Windows worsenings

Disposition: fixed in the record. The finding holds on both points: by Q7's rule (median after below before, and below
in most rounds) W6 (0.998, slower in 6 of 10) and W19 (0.995, 2 of 3) also get worse on Windows, which the summary had
put under «the rest»; in WSL, `ab2.sh` labels W12's two results (W12 and W13) as W12, so they were mixed. Recalculated
with Q7's rule and the median of each round's repetitions, W12 and W13 apart (L14): Windows worsens W6, W19 and W23, by
0.2 %, 0.5 % and 0.2 %; WSL worsens nothing after the second measurement (W13 1.001, slower in 12 of 20). W6, W19 and
W23 are measured again on Windows in 20 rounds (L14); the closing report lists what still gets worse for the user's
decision, as Q7 says.

Status: claimed, unverified until a follow-up review.

Addendum to R3, same date: the 20-round Windows measurement (`work/wm/q7-d.json`) gives W6 0.997 (slower in 11 of 20),
W19 1.001 (10 of 20) and W23 1.005 (13 of 20). By Q7's rule only W6 still gets worse, by 0.3 %; the closing report
lists it for the user's decision (L14).

## 2026-11-17 (verification, round 3)

Follow-up report: [2026-11-17-code-03.md](2026-11-17-code-03.md), fix range `df8abb1..8bff765`, verdict «Approved
within the requested follow-up scope.» Per ID, quoted: R5 «Verified resolved: colliding transfer suppressing Q4
validation»; R3 «Verified resolved: inaccurate Q7 assessment». R1, R2 and R4 stay verified resolved (round 2). Open
question, for the user after the loop: «¿Aceptás cerrar el cambio con la caída medida de W6 en Windows del 0,33 %,
según lo previsto en Q7?»

Review record: [2026-11-17-code-03.md](2026-11-17-code-03.md), wrapper status `approved`, thread
`01a1e8dd-8535-7b92-b250-a253490bd0e3`. Telemetry: coordinator `gpt-6.1-sol` / `high`; reviewer
`krummi-review-critical` (Poincare) `gpt-6-astra` / `xhigh`.

## 2026-11-17 (note on round 3)

The open question in «(verification, round 3)» is my Spanish translation, not a quotation. The report's words
(`2026-11-17-code-03.md:107`): «Q7 reserves acceptance of the measured Windows W6 tradeoff (approximately 0.33% lower
throughput in the rerun) to the user. Does the user accept closing the change with this result?» (RDD R2-002,
lineage `review-a242d2b9304e896e`).
