# A faster xxh3 through run-time CPU detection (O3a)

Objective: O3a of [the performance plan](../../docs/performance/plan.md): the pager's xxh3-64 picks its SIMD variant
at run time. Branch `perf/fast-xxh3` from `main` `c245fee`. Delivery: single branch, unmerged. Test runner:
`cargo test`. No PRD: an optimization without an observable change (K13 annotation of 2026-11-04).

## Specs

- **S1** (plan, O3a, verbatim): "`xxhash-rust` 0.8.19 picks its SIMD variant of xxh3 at compile time
  (`cfg(target_feature)`), so the default `x86-64` target gets SSE2 even on CPUs with AVX2. `twox-hash` 2.1.5
  (features `std`, `xxhash3_64`) compiles AVX2, SSE2 and scalar variants and picks one at run time
  (`is_x86_feature_detected!`, its `dispatch` in `src/xxhash3/large.rs`). Replace the crate in `krummi-pager`."
- **S2** (plan, O3a, invariants, verbatim): "every checksum is the same value as today (the stored format, G6); a
  test compares the new crate with recorded values of the old one."
- **S3** (plan, O3a, targets, verbatim): "every load: reads verify pages and writes seal them. It does not close W7
  on its own."
- **S4** (acceptance, as for O1 and O2): single-load worker runs of W7–W12 before and after, at full scale, on
  Windows and WSL 2, each load in its own `measure.py` call, alternating the binaries (L7 of
  [scratch-lock.md](scratch-lock.md)); the complete runs of the plan's *Accepting an optimization* run once after
  O1, O2 and O3a.

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S2 | inline: test that pins xxh3-64 to values recorded with `xxhash-rust` | pending | |
| T2 | S1 | inline: replace `xxhash-rust` with `twox-hash` in `krummi-pager` | pending | |
| T3 | S3, S4 | inline: single-load runs before and after, Windows and WSL 2 | pending | |
| T4 | S2 | independent verifier (risk items 1 and 5), RDD, Codex, closing | pending | |

## Log

- **L1** (2026-11-05): user's request, verbatim: "Adelante." to "Lo que sigue en el plan es O3a (twox-hash)."
- **L2** (2026-11-05): estimate 1 h 30 min – 2 h 30 min, confirmed ("Sí"); start 12:56:19 UTC.
- **L3** (2026-11-05): map. `xxh3_64` is called in `format.rs` (page and run checksums, both header versions) and
  `journal.rs` (record header and payload); nothing else in the workspace uses `xxhash-rust` except the benchmark's
  competitors (`canopydb`, `fjall`, `lsm-tree`, `sfa`), which keep it. `twox-hash` 2.1.5 is already in the lock
  through `lz4_flex`. No test pins checksum values today. Risk: item 1 (the checksum is part of the stored format;
  a different value makes every existing store unreadable) and item 5 (a dependency changes): independent verifier.
- **L4** (2026-11-05): T1 in `f034f8f`. Test `xxh3_keeps_the_values_of_the_stored_format` (`format.rs`): 16 values
  printed by `xxhash-rust` 0.8.19 at `c245fee`, for lengths at the edges of each xxh3 input class (0, 1, 3, 4, 8,
  9, 16, 17, 128, 129, 240, 241), a journal record header (56), a page without its checksum (8,184), a page (8,192)
  and a run of 12 pages (98,304); the input is built from 64-bit arithmetic so it is the same on every target. The
  empty input gives `0x2d06800538d394c2`, xxh3-64's published value for it. Passes with the old crate.
- **L5** (2026-11-05): T2 in `8c80d79`. `twox-hash` 2.1.5 (`default-features = false`, features `std` and
  `xxhash3_64`) replaces `xxhash-rust` in `krummi-pager`; `format.rs` gets `pub(crate) fn xxh3_64`, the one place
  that names the crate, and `journal.rs` imports it. The T1 test passes with the new crate. Mutation check, not
  committed: with `oneshot_with_seed(1, ..)` it fails (`0 bytes`). Only the AVX2 path runs on this machine (both
  platforms share the CPU), so the test does not reach the SSE2 or scalar paths here.
- **L6** (2026-11-05): independent verifier (sonnet/high) over `c245fee..8c80d79`: S1, S2 and S3 PASS, S4 left to
  the measurements, no blockers. Every former `xxh3_64` call now goes through `format::xxh3_64` with the same
  slices; no other crate of the engine hashes stored data. A probe outside the workspace compared
  `XxHash3_64::oneshot` with `xxhash_rust::xxh3::xxh3_64` on 25,266 inputs (lengths 0 to 4,200 and up to 1 MiB + 13,
  three contents, aligned and offset), with twox-hash's dispatch automatic and forced to AVX2, SSE2 and scalar
  (`RUSTFLAGS="--cfg _internal_xxhash3_force_<variant>"`): 0 mismatches in each build; the 16 recorded constants
  recomputed with `xxhash-rust` 0.8.19 from `c245fee`: 0 mismatches. `std` is what enables the run-time detection
  (`all(target_arch = "x86_64", feature = "std")`); `lz4_flex` asks twox-hash only for `xxhash32`, behind its
  `frame` feature, which Krummi does not enable. Its checks: fmt ok, workspace clippy clean, `cargo test -p
  krummi-pager` passed. Advisory kept out: running the test under the forced SSE2 and scalar cfgs in CI; those cfgs
  are internal to twox-hash and may change. Verifier scratch removed.
