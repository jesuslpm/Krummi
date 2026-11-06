# Fewer zeroed buffers and allocations in the commit (O4)

Objective: O4 of [the performance plan](../../docs/performance/plan.md): the commit stops zeroing journal buffers
that it overwrites and reuses the compression buffer across commits. Branch `perf/commit-copies` from
`perf/integrity-once` `13489d1`. Delivery: single branch, unmerged. Test runner: `cargo test`. No PRD: an
optimization without an observable change (K13 annotation of 2026-11-04).

## Specs

- **S1** (plan, O4, verbatim): "the commit copies each changed page several times (sealing, the temporary area, the
  journal record, the group write) and allocates zeroed buffers per record and per group. Reuse buffers across
  commits, skip the zeroing of buffers that are fully overwritten, and remove the copies that only move bytes
  between stages. Keep the journal's format and its compression (J16)."
- **S2** (plan, O4, invariants, verbatim): "the journal record bytes and the data file bytes are the same as today
  for the same writes; a failure at any stage leaves the store as today."
- **S3** (plan, O4, targets, verbatim): "W1–W3, W13–W15, W17, W19, W21."
- **S4** (acceptance, as for O1–O3): single-load runs of the target write loads before and after, Windows and WSL 2;
  the complete runs after O4 (L2 of [integrity-once.md](integrity-once.md)).

## Tasks

| ID | Specs | Route | Status | Evidence |
|---|---|---|---|---|
| T1 | S2 | inline: pin the bytes of journal records (compressed, raw, several pages) to values recorded before the change | done | `489bd4e`, L5 |
| T2 | S1, S2 | inline: build records and group writes without zeroing what they overwrite; reuse the compression buffers across commits | done | `cf61713`, L6 |
| T3 | S3, S4 | inline: single-load runs of the write loads, before and after, Windows and WSL 2 | done | L7 |
| T5 | S4 | inline: complete runs on Windows and WSL 2 after O4 | done | L10 |
| T4 | all | RDD, Codex, closing; independent verifier only if a risk item applies | done; Codex R1 pending for the user | L8–L12, [report](commit-copies-report.md) |

## Log

- **L1** (2026-11-06): user's request, verbatim: "Haz O3, luego O4, por último el benchmark." Estimate and start
  are those of O3 (L2 of [integrity-once.md](integrity-once.md)).
- **L2** (2026-11-06): map (explorer, during O3's Codex review). Per changed page the commit zeroes a sealed page
  (`alloc_sealed`), the LZ4 output buffer (`encode_record`, `resize(bound, 0)`), the record (`AlignedBuf::zeroed`)
  and the group buffer (`write_group`, `AlignedBuf::zeroed`), and copies the page into the sealed page, the
  temporary area, the raw journal payload, the record and the group buffer. The plan's profile of W2 and W21 puts
  LZ4 at 32.5 % / 14.9 %, the copies at 14.0 % / 26.6 % and the zeroed allocations at 7.1 % / 17.5 %, the last two
  mostly in `enqueue` and `write_group`. No test pins journal bytes. `AlignedBuf::zeroed` and the `Vec` reservations
  of `encode_record` are no fault points (`Op::Alloc`), so changing them shifts no injected failure.
- **L3** (2026-11-06): scope. Taken: (a) records and group buffers are built by copying their parts into
  uninitialized aligned memory and zeroing only the padding; (b) the raw payload and the LZ4 output buffer are kept
  by the journal across commits, the output zeroed only when it grows, and dropped after a commit larger than the
  queue limit (8 MiB) so a large commit does not keep its memory. Left out, as larger changes for a later step:
  sealing pages in place (removing `alloc_sealed` and its copy), writing a single-record group without its copy,
  and vectored writes (they change the journal's write counts). Risk: item 1 only if bytes change, which T1 pins;
  no concurrency change (the buffers are used under the journal's enqueue, by the one writer).
- **L4** (2026-11-06): Engram mirror pending: the memory server rejected writes (its session ended) since the O3
  closing; this file is the record.
- **L5** (2026-11-06): T1 in `489bd4e`. `records_keep_the_bytes_of_the_stored_format` (`journal.rs`): xxh3 of five
  whole records (padding included) recorded with the code before O4: one compressible page, one half compressible,
  one incompressible (stored raw), three pages, four pages. Passes at the base.
- **L6** (2026-11-06): T2 in `cf61713`. `AlignedBuf::from_parts` copies the parts into uninitialized aligned memory and
  zeroes only what follows them; `encode_record` builds the record with it, and `write_group` the group buffer and
  its per-record fallback. `EncodeBuffers` (raw payload and LZ4 output) live in the journal under a mutex used only
  by the enqueueing commit, the output zeroed only when it grows; after a commit beyond 8 MiB both give their memory
  back. The pinned record bytes still pass with shared buffers across the cases;
  `reused_buffers_build_the_same_records_as_new_ones` compares shared and new buffers, a small record after large
  ones and a commit beyond the kept size included; `an_aligned_buffer_from_parts_holds_them_and_then_zeros` pins
  the helper. Group writes keep their bytes: each record copied whole, then its header's `durable_upto` set, as
  before. Checks: fmt ok, clippy clean, `cargo test --workspace` 546 passed, 0 failed, 30 ignored.
- **L7** (2026-11-06): T3. Single-load runs (`work/o4/measure.py`: median of reps 1–4 in operations per second, each
  load in a fresh store), `before` = `13489d1`, `after` = `cf61713`, two rounds of W2, W4, W14, W17 and W21 with the
  order of the binaries swapped in the second; ratio after / before per round:

  | Load | Windows | WSL 2 |
  |---|---|---|
  | W2 | 1.035, 1.093 | 0.979, 1.026 |
  | W4 | 1.057, 1.026 | 0.75, 0.85; rounds 3–4: 0.95, 1.01 |
  | W14 | 1.096, 1.091 | 1.011, 1.021 |
  | W17 | 0.80, 0.93; rounds 3–4: 1.17, 0.99 | 1.19, 1.04 |
  | W21 | 1.170, 1.196 | 1.055, 1.001 |

  The two loads that came out lower in both first rounds got two more: W17 on Windows (16 MiB commits, reps from
  1,216 to 2,357) and W4 on WSL (one flushed commit per entry, about 100 per second, `before` reps from 75 to 161)
  scatter beyond any change and land on both sides in rounds 3–4, so they read as noise, not a loss. Gains: W21 and
  W14 on Windows (+17–20 %, +9–10 %), the loads with the most pages per commit; the rest within noise. W1, W3, W13,
  W15 and W19 were not run singly: they share the commit path of the measured ones and the complete runs measure
  them (S4). Raw output: `work/o4/win.out`, `wsl.out`, `win-extra.out`, `wsl-extra.out` (not committed).
- **L8** (2026-11-06): RDD review of `c5062a4..ba2a3ae` (O3's last code fix `bcb71ce` and O4): lenses risk,
  resilience, reliability; approved, lineage `review-8c24a99484b62d18`, acknowledged. Findings:
  - R4-001 (warning): an open store keeps up to 8 MiB of raw payload and about as much LZ4 output between commits.
    Accepted: the bound is the journal's queue limit, which already lets a store hold 8 MiB of queued records, and a
    larger commit gives the memory back; recorded as a trade-off for the closing report.
  - R3-group-parts-vec-infallible-alloc (warning): `write_group` collected the list of parts infallibly, so no memory
    for it aborted instead of taking the per-record fallback. Fixed in the next commit: the list is reserved with
    `try_reserve_exact` and its failure takes the fallback. No test: allocation failure cannot be injected there
    (`Op::Alloc` covers no `Vec`), as before O4. Checks: fmt ok, clippy clean, 546 passed, 0 failed, 30 ignored.
  - R3-stale-run-mark-after-overlong-length (suggestion, O3): a length beyond the mark leaves the slot's earlier mark;
    safe (a mismatch checks), left as is.
  - R3-r1-residual-open (warning, O3): Codex R1, already pending as a user decision (O3 resolutions).
- **L9** (2026-11-06): RDD review of the last slice `ba2a3ae..4e1e38d` (the fallible list of parts): lens
  resilience; approved without findings, lineage `review-998d9967b748de57`, acknowledged. Codex R1
  (`-Range 13489d1..4e1e38d`) ended in `error` at 11:36 UTC: Codex's usage limit, until 13:30; no report, no
  change. Default taken: the complete runs (S4) go first, at `4e1e38d`, and the Codex review after them, so that
  neither disturbs the other; a Codex fix that touches the commit path would mean running them again.
- **L10** (2026-11-06): T5, the complete runs at `2fdf8e9` (the only uncommitted file was the phase log), Windows
  11:39–13:06 and WSL 2 13:07–15:26 UTC: [Windows](../../docs/benchmark/2026-11-06-windows.md) and
  [Linux](../../docs/benchmark/2026-11-06-linux.md), 10 of 22 thresholds on both; the comparison with 2026-11-05 is
  L16 of [integrity-once.md](integrity-once.md) and the plan's History. The loads whose Krummi op/s fell against
  2026-11-05 were run singly, two rounds with the order swapped, `pre` = `3ec8641` (before O3) and `head` = the
  O4 head build of the complete run (`work/o4/full.sh`); ratio head / pre per round: Windows W10 1.05, 1.10; WSL 2
  W2 1.02, 1.03; W14 1.00, 1.02; W15 1.01, 1.05; W16 1.01, 0.68 (reps 254–606); W17 1.09, 1.03.
- **L11** (2026-11-06): Codex R1 ([report](../../docs/reviews/commit-copies/2026-11-06-code.md),
  [resolutions](../../docs/reviews/commit-copies/2026-11-06-code-resolutions.md)): `findings`, one blocker, R1: by
  K13's annotation of 2026-11-04, O4's acceptance is that its target loads meet their threshold in the complete runs,
  and none of the nine does; and S4's single-load runs left out W1, W3, W13, W15 and W19. The missing runs were made
  (`work/o4/rest.sh`, same binaries and method as L7; W13 runs in W12's process, so it was measured as W12); ratio
  after / before per round:

  | Load | Windows | WSL 2 |
  |---|---|---|
  | W1 | 0.997, 1.039 | 1.011, 1.011 |
  | W3 | 0.958, 1.014 | 1.019, 0.994 |
  | W13 | 1.016, 1.109 | 1.066, 1.063 |
  | W15 | 1.015, 1.036 | 0.843, 1.043 |
  | W19 | 1.054, 1.048 | 1.014, 1.040 |
  | W12 (not a target, same process) | 0.963, 0.965 | 0.968, 1.008 |

  WSL W15's first round has `before` reps from 27,939 to 35,059 and `after` from 25,314 to 34,415. The thresholds
  part of R1 is a product decision, pending for the user: O1, O2 and O3a closed as steps with no threshold met yet,
  but K13 does not say so.
- **L12** (2026-11-06): Codex follow-up ([report](../../docs/reviews/commit-copies/2026-11-06-code-02.md)): the
  missing runs verified on Windows; WSL's result files then copied to `work/o4/`; the thresholds part stays open.
  The loop stops at two rounds. No independent verifier: risk item 1 only if the bytes change, and the golden hashes
  pin them. Closing report: [commit-copies-report.md](commit-copies-report.md).
