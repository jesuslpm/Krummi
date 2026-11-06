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
| T1 | S2 | inline: pin the bytes of journal records (compressed, raw, several pages) to values recorded before the change | pending | |
| T2 | S1, S2 | inline: build records and group writes without zeroing what they overwrite; reuse the compression buffers across commits | pending | |
| T3 | S3, S4 | inline: single-load runs of the write loads, before and after, Windows and WSL 2 | pending | |
| T4 | all | RDD, Codex, closing; independent verifier only if a risk item applies | pending | |

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
