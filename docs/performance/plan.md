# Performance plan: meeting the benchmark thresholds

Technical guide for the optimizations that bring Krummi to at least 0.80 × Voron's operations per second on every
load of the comparative benchmark (K7 of [benchmark.prd.md](../prd/krummi/benchmark.prd.md)). It is not a PRD:
under the annotation of 2026-11-04 to K13, an optimization that changes nothing observable of the product runs
straight as an ODD execution, and one that does change something observable (output, messages, stored data or its
format on disk, the public interface, the guarantees or the options) needs its own PRD first. Each optimization
below says which of the two it is.

Each ODD execution takes one optimization of [Optimizations](#optimizations): its feature document quotes the
optimization's target loads and goal verbatim as its specs, and its closing updates [History](#history).

## Starting point

The first complete reports, 2026-11-03: [Windows](../benchmark/2026-11-03-windows.md) (5 of 22 thresholds met) and
[Linux, WSL 2](../benchmark/2026-11-03-linux.md) (7 of 22). Ratio Krummi / Voron of the missed loads:

| Group | Load | Windows | Linux |
|---|---|---|---|
| Reads | W7 `lecturas-al-azar` | 0.41 | 0.42 |
| | W8 `lecturas-al-azar-4` | 0.35 | 0.37 |
| | W9 `lecturas-al-azar-16` | 0.13 | 0.15 |
| | W11 `intervalos` | 0.58 | 0.63 |
| | W12 `lecturas-con-escritor` | 0.22 | 0.32 |
| | W18 `valores-grandes-lectura` | 0.58 | 0.60 |
| | W22 `tamano-fijo-lectura` | 0.20 | 0.25 |
| Writes | W1 `carga-en-orden` | 0.48 | met (0.86) |
| | W2 `carga-al-azar` | 0.24 | 0.38 |
| | W3 `carga-masiva` | 0.57 | 0.54 |
| | W4 `escrituras-sueltas` | 0.74 | met (1.00) |
| | W13 `escritor-con-lectores` | 0.25 | 0.65 |
| | W14 `borrado` | 0.26 | 0.48 |
| | W15 `reutilizacion` | 0.27 | 0.39 |
| | W17 `valores-grandes-carga` | 0.27 | 0.63 |
| | W19 `multivalor-carga` | 0.23 | 0.36 |
| | W21 `tamano-fijo-carga` | 0.31 | 0.42 |

On Linux the single and concurrent durable writes (W4–W6) meet their thresholds because every engine waits about
6 ms per durable commit on WSL 2, which hides the CPU cost per write; on Windows the same CPU cost shows.

## Measurement method

- **Profile one load, not the benchmark.** The Rust worker runs a single load at full scale:
  `KRUMMI_BENCH_ROLE=worker krummi-bench run krummi <load> full <store> [<source>]` (protocol in
  `crates/krummi-bench/src/protocol.rs`). Prepare the base store once with `prepare krummi full <base>`; a load
  that starts from the base takes it as `<source>`, W18, W20 and W22 take the last store of W17, W19 and W21
  (`<store>/rep-3`). A read load takes seconds, against about two hours for a complete run.
- **Profiler.** samply 0.13.1 (`cargo install --locked samply`). On Linux (WSL 2) it needs
  `/proc/sys/kernel/perf_event_paranoid` at 1 or lower, set as root
  (`wsl -d Ubuntu -u root -- sh -c "echo 1 > /proc/sys/kernel/perf_event_paranoid"`; it returns to 2 when WSL
  restarts). On Windows it records through ETW and needs administrator rights. Build with line tables so that
  inlined functions are attributed: `CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`, in a separate target folder.
  Record with `samply record --save-only --unstable-presymbolicate -o <load>.json.gz -- <worker command>`.
- **Reading a profile.** Open it with `samply load <load>.json.gz`, or aggregate the self and inclusive share per
  function from the profile and its `.syms.json` sidecar (inline frames expanded).
- **Accepting an optimization.** The complete runs on Windows and Linux are the acceptance (K13 annotation of
  2026-11-04): the target loads meet their threshold and no other load gets worse beyond noise. An optimization may
  close as an intermediate step when single-load runs before and after measure what it improves, even if its target
  loads do not meet their threshold yet; the threshold stays the goal of the next ones (K13 annotation of
  2026-11-06). Noise: compare with the previous
  complete report of the same platform. A load whose ratio drops by more than 0.05 counts as worse only when
  single-load runs, before and after in the same session with the order alternated, confirm the drop. Complete runs
  on different days move by more than 0.05, also for the engines that do not change (annotation of 2026-11-06 to
  AC-I12 of [pager-integrity-once.prd.md](../prd/krummi/pager-integrity-once.prd.md)). While iterating, the single-load worker run measures
  before and after.

## Diagnosis

### Reads (W7, W9; profiled on WSL 2 at `ba90afd`, 2026-11-04)

The path of one `get`: `krummi::ReadTransaction::get` → `krummi_tree::get` → `descend`, one `node_at_with` per
level (`crates/krummi-tree/src/lib.rs:236-265`) → `Snapshot::page_and_code`
(`crates/krummi-pager/src/scratch.rs:993-1022`) → `Pager::page_and_code` → `Pager::run`
(`crates/krummi-pager/src/pager.rs:293-340`) → `raw_pages` (`pager.rs:370-393`) and `format::verify_run`. Pages
are read through the memory map, without copies; there is no page cache of Krummi's own. The benchmark adapter does
the same work per operation as LMDB's, so the gap is inside the engine.

| Cost per page access | Where | W7 (1 thread) | W9 (16 threads) |
|---|---|---|---|
| xxh3 over the whole 8 KiB page (`verify_run`) | `format.rs:275-280` | 62.4 % | 18.5 % |
| Pager mutex `FileState`, to read `len` and the last mapping | `pager.rs:375` | small (uncontended) | 51.2 % (`syscall` of the futex 50.9 %) |
| Scratch mutex `State` and the `versions` hash map lookup | `scratch.rs:994-998` | in `page_and_code`, 13.5 % self | in `lock_contended`, part of 32.5 % |
| Binary search in the node | `krummi-tree/src/node.rs:278-289` | 13.2 % | 4.7 % |

- **The gap at 1 thread** comes from the checksum: about three pages per `get` (a tree of about 1M entries has
  three levels, an estimate), so about 24 KiB hashed per `get`. LMDB and Voron do not verify pages on read.
- **The negative scaling** (W8 → W9: 1.15 M → 0.41 M op/s on Windows) comes from the two process-wide mutexes
  taken on every page access, the pager's above all: 16 threads spend half their samples waiting on its futex.
- The same path serves W11 (each range re-descends from the root), W12 (readers beside a writer, where the scratch
  `versions` map is not empty), W18 (value runs) and W22 (fixed-size trees, `fixed.rs:215`, `:366`, `:392`, `:432`).

W22 (fixed-size gets, profiled the same day) confirms it: 76.0 % of its samples are the page checksum.

### Writes (W2, W21; profiled on WSL 2 at `ba90afd`, 2026-11-04)

The commit takes 55.5 % of W2's samples and 59.4 % of W21's (`WriteTransaction::commit_with`,
`crates/krummi-tx/src/transaction.rs:415`); changing the tree takes about 24 %. Inside the commit:

| Cost | Where | W2 | W21 |
|---|---|---|---|
| LZ4 compression of the journal record | `encode_record`, `crates/krummi-pager/src/journal.rs:174-184` | 32.5 % | 14.9 % |
| Copies (`memmove` of libc) in `enqueue`, `store_run`, `seal_page` and `write_group` | `journal.rs:434`, `temp.rs:160`, `format.rs:226`, `journal.rs:709` | 14.0 % in all (38 % of it in `enqueue`) | 26.6 % in all (46 % in `enqueue`) |
| Zeroed allocations (`alloc_zeroed`, `memset` of libc) in `enqueue` and `write_group` | same | 7.1 % in all | 17.5 % in all |
| Page checksum (sealing written pages, reading on the way down) | `format.rs` | 9.5 % | 10.4 % |
| `pwrite` and `fsync` | — | 14.7 % | 5.7 % |

- libc has no symbols in these profiles: the copies and the zeroed allocations are two unnamed libc functions,
  identified by their callers (`alloc_zeroed` calls the second one); confirm with libc symbols before O4.
- The journal is compressed by product rule (J16 of [journal.prd.md](../prd/krummi/journal.prd.md)), as Voron's
  is; removing the compression changes stored data and needs a PRD. Making it cheaper (compressing into a reused
  buffer, not zeroing the output buffer before `compress_into`) does not.
- A W2 repetition takes about 37 s for 1M inserts in 1,000 writes, so the CPU per write dominates over the 1,000
  durable commits (about 6 ms each on WSL 2).

### Writes after O1–O4 (profiled on WSL 2 at `0fd0fdb`, 2026-11-06)

After the complete runs of 2026-11-06, every read load meets its threshold except W11, W12 and W18. No write load
meets it except W4–W6. Ratios, Windows / Linux:

| Load | Windows | Linux |
|---|---|---|
| W1 | 0.47 | 0.76 |
| W2 | 0.24 | 0.31 |
| W3 | 0.53 | 0.55 |
| W13 | 0.41 | 0.73 |
| W14 | 0.35 | 0.39 |
| W15 | 0.27 | 0.35 |
| W17 | 0.39 | 0.60 |
| W19 | 0.31 | 0.35 |
| W21 | 0.49 | 0.44 |

Eight write loads were profiled at full scale with the method above. The profiles are in `~/prof/` of WSL and the
aggregator is `work/prof/agg.py`; neither is in git. The shares are of the samples of the committing thread. Windows
was not profiled: samply needs administrator rights there.

| Load | Commit (`commit_with`) | Tree change | Biggest costs |
|---|---|---|---|
| W2 | 66.2 % | 33.2 % | LZ4 39.9 %, copies 16.2 %, node layout check 12.1 %, page checksum 8.7 % |
| W15 | 64.7 % | 34.1 % | LZ4 37.7 %, copies 16.1 %, page checksum 13.0 %, node layout check 5.0 % |
| W14 | 62.1 % | 37.0 % | LZ4 36.1 %, copies 16.2 %, page checksum 10.9 %, node layout check 8.7 % |
| W19 | 55.2 % | about 45 % | LZ4 37.9 %, node layout check about 12 %, copies 10.5 %, page checksum about 4.8 % |
| W21 | 59.0 % | 40.2 % | copies 29.2 %, LZ4 22.8 %, page checksum 12.3 % |
| W17 | 72.3 % | 12.3 % | copies 37.8 %, zeroing 13.1 %, LZ4 8.1 %, page checksum 7.3 % (few samples) |
| W1 | 9.8 % | 88.2 % | node layout check 63.4 %, LZ4 6.4 %, page checksum 4.9 % |
| W3 | 14.3 % | 81.7 % | node layout check 57.0 %, copies 8.4 %, LZ4 3.7 % |

- **LZ4.** Compression (`encode_record_in`, `crates/krummi-pager/src/journal.rs:216`) runs on the committing thread
  over whole 8 KiB pages. Voron instead journals each changed page as a diff against its version before the
  transaction: 8-byte words, skipping unchanged runs. It then compresses the diff (`WriteAheadJournal.cs:1864-1885`,
  `:1949` of RavenDB 7.2.7). `CompressTxAboveSizeInBytes` is 0 by default, so Voron always compresses. A random insert
  changes one cell and a few offsets of an 8 KiB leaf, so Voron compresses and writes far fewer bytes (inferred: 10
  to 30 times fewer).
- **Copies.** The libc `memmove` is unsymbolized and named here by its callers. It stays at 15–29 % (38 % in W17),
  split about evenly among four callers:
  - `enqueue`: the raw payload.
  - `seal_page` (`format.rs:232`).
  - `store_run` (`temp.rs:160`).
  - `AlignedBuf::from_parts`: the record.

  Between `page_mut` and the journal write, Krummi copies each changed page five or six times: `copy_for_write`, seal,
  stage, raw payload, record and group buffer. Voron copies it once, into its scratch area.
- **Node layout check.** `node::is_valid` and `cell_len_at` (`crates/krummi-tree/src/node.rs:109`, `:196`) are called
  from `node_at_with` (`crates/krummi-tree/src/lib.rs:256`). A page the transaction changed has no integrity code,
  so its layout is checked in full on every read (C3 of
  [trees-node-check-cache.prd.md](../prd/krummi/trees-node-check-cache.prd.md)). A sequential load reads the same
  few changed leaves and branches thousands of times per transaction, so this check takes 57–63 % of W1 and W3. The
  2026-11-04 profile missed it because it covered only W2 and W21.
- **Page checksum on the way down.** `page_and_code` → `run_with` (`scratch.rs:999`, `pager.rs:382`) takes
  5.7–10.2 %, more than sealing (2.7–5.0 %). The writer checks every page it reads, even a remembered one (I3 of
  [pager-integrity-once.prd.md](../prd/krummi/pager-integrity-once.prd.md)), so a clean leaf can be hashed up to
  three times per insert: on the descent, in `insert_cell` and in `copy_for_write`. Voron checks each data file page
  once per process (`StorageEnvironment.cs:1561-1576`).
- **Against 2026-11-04.**
  - Zeroing fell from 7.1 % to 2.9 % in W2 and from 17.5 % to 1.6 % in W21: that is O4.
  - LZ4's share rose (32.5 % → 39.9 % in W2, 14.9 % → 22.8 % in W21) because the other costs shrank.
  - Copies did not fall.
  - `pwrite` and `fsync` run on the journal thread and take under 0.5 % of the committing thread.
  - No lock or allocator hotspot appeared.

## Optimizations

### O1 — Read the pager's mapping without a lock

- **Kind:** optimization without observable change → ODD, no PRD.
- **What:** `raw_pages` and `len` take `FileStorage.state` on every page access only to read `len` and
  `maps.last()`. Both change only when the store grows, under the writer. Publish them so that readers get them
  without a lock (for example an atomic length and a pointer to the newest mapping, with the old mappings kept alive
  until the store closes, as today).
- **Targets:** W8, W9 above all; it lowers the cost of every read load.
- **Invariants to keep:** a reference into a mapping stays valid for the life of `&Pager` (the SAFETY comment at
  `pager.rs:386-391`); a reader never sees a length beyond what the mapping it gets covers.
- **Risk:** concurrency (item 4 of the high-risk list): it gets an independent verifier and its own RDD review.

### O2 — Avoid the scratch lock when no version can apply

- **Kind:** optimization without observable change → ODD, no PRD.
- **What:** `Snapshot::page_and_code` locks the scratch `State` and looks up `versions` on every page access, even
  when the map is empty (all the read-only loads). Decide without the lock whether any version can apply to this
  snapshot, or make the lookup lock-free.
- **Targets:** W7–W9, W11, W18, W22; W12 needs the lookup to stay correct while the writer publishes versions.
- **Risk:** concurrency (item 4): verifier and own review.

### O3a — A faster xxh3 through run-time CPU detection

- **Kind:** optimization without observable change → ODD, no PRD.
- **What:** `xxhash-rust` 0.8.19 picks its SIMD variant of xxh3 at compile time (`cfg(target_feature)`), so the
  default `x86-64` target gets SSE2 even on CPUs with AVX2. `twox-hash` 2.1.5 (features `std`, `xxhash3_64`)
  compiles AVX2, SSE2 and scalar variants and picks one at run time (`is_x86_feature_detected!`, its `dispatch`
  in `src/xxhash3/large.rs`). Replace the crate in `krummi-pager`.
- **Measured** (WSL 2, i9-13980HX, 2026-11-04, 64 pages of 8 KiB hashed 200,000 times): both crates give the same
  values; `xxhash-rust` default 22 GB/s (about 370 ns per page); `twox-hash` 40 GB/s (about 205 ns); `xxhash-rust`
  built with `+avx2` 43 GB/s. W7 at full scale: SSE2 build 470,000–510,000 op/s, AVX2 build 490,000–670,000
  op/s; the repetitions are noisy, about +15 % to +25 %.
- **Invariants to keep:** every checksum is the same value as today (the stored format, G6); a test compares the
  new crate with recorded values of the old one.
- **Targets:** every load: reads verify pages and writes seal them. It does not close W7 on its own.

### O3 — Stop verifying the checksum of the same page on every read

- **Kind:** **changes a guarantee → needs a PRD first.** The remembered node layout check (5.x) kept the integrity
  check on every read of the data file (C7 of [trees-node-check-cache.prd.md](../prd/krummi/trees-node-check-cache.prd.md))
  and left out of its scope "Dejar de comprobar la integridad en cada lectura, o recordarla".
- **What:** verify each page of the data file once per opening and remember it, as Voron does: its
  `_validPagesAfterLoad` bitmap marks a page after its first successful check, and pages beyond the file's size at
  load are not checked, since this opening wrote them (`src/Voron/StorageEnvironment.cs:1559-1600` of RavenDB
  7.2.7). Krummi already remembers checked layouts per opening (C1–C5); the PRD extends that to integrity and
  accepts the residual risk C7 describes for the layout. Alternative to weigh in the PRD: verify once per
  transaction.
- **Decision:** the user chose both O3a and O3 (2026-11-04).
- **Targets:** W7 above all (62 % of its samples); every read load.

### O4 — Fewer copies and zeroed buffers in the commit

- **Kind:** optimization without observable change → ODD, no PRD.
- **What:** the commit copies each changed page several times (sealing, the temporary area, the journal record,
  the group write) and allocates zeroed buffers per record and per group. Reuse buffers across commits, skip the
  zeroing of buffers that are fully overwritten, and remove the copies that only move bytes between stages. Keep the
  journal's format and its compression (J16).
- **Targets:** W1–W3, W13–W15, W17, W19, W21.
- **Invariants to keep:** the journal record bytes and the data file bytes are the same as today for the same
  writes; a failure at any stage leaves the store as today.
- **Risk:** data and irreversible effects only if the bytes change (item 1); the byte-for-byte invariant is what its
  tests pin.

### O5 — Copy each changed page once in the commit

- **Kind:** optimization without observable change → ODD, no PRD.
- **What:** O4 left these copies out (L3 of [commit-copies.md](../../odd/tasks/commit-copies.md)). Remove them:
  - Seal each changed page straight into its slot of the temporary area, instead of into a separate sealed page.
  - Build the journal record from those slots, without copying the raw payload.
  - Write a group without copying its records into a second buffer: lay the records out in place in one aligned
    buffer, or use vectored writes.
  - Compress with `lz4_flex`'s encoder without `safe-encode`. Measured on the 22,080 pages of a base store, in
    records of 1,000 pages: 1,691–1,788 MB/s against 1,133–1,315 MB/s, with identical output. Allowed by the
    annotation of 2026-11-06 to D12 of [krummi.prd.md](../prd/krummi/krummi.prd.md). The same pages compress to
    0.756 of their size.
- **Targets:** W2, W14, W15, W17, W19, W21.
- **Invariants to keep:**
  - The journal record bytes and the data file bytes stay the same as today. O4's golden record hashes pin the
    record bytes.
  - A failure at any stage leaves the store as today.
- **Risk:** item 1 only if the bytes change; item 4 if the temporary area is written while readers may read it.

### O6 — Check the layout of a changed node once per change

- **Kind:** changes C3 of [trees-node-check-cache.prd.md](../prd/krummi/trees-node-check-cache.prd.md), under which
  the pages the write itself changes are checked on every read → PRD amendment first.
- **What:** a node the transaction changed is checked the first time it is read after its last change. That check
  is remembered until the transaction changes the node again. The memory is per transaction, because it covers only
  the transaction's own pages. Pages that other transactions confirmed, in the temporary area, keep C3 as it is.
- **Targets:** W1 and W3 (57–63 %), and W2, W14 and W19 (5–12 %).
- **Invariants to keep:**
  - A node written by a damaged code path is still caught before it is used after its change.
  - The check of store pages (C1–C2) does not change.
- **Risk:** item 6, because no test would catch a remembered check that survives a change. Its tests change a
  checked node and read it again.

### O7 — Hash each store page once per write transaction

- **Kind:** changes I3 of [pager-integrity-once.prd.md](../prd/krummi/pager-integrity-once.prd.md), under which the
  writer checks every page it reads, on every read → PRD amendment first. The lighter form below also changes how
  I3 reads, so it goes in the same amendment.
- **What:** two forms:
  - Lighter: the writer checks a store page the first time the transaction reads it, and trusts the same bytes for
    the rest of the transaction. That is one check per page per transaction instead of up to three per insert.
  - Fuller: like Voron and the readers, the writer uses the remembered pages.
- **Targets:** W2, W14, W15 and W21 (5.7–10.2 %), and every write load a little.
- **Invariants to keep:** no damaged page is sealed into a new version, which is the reason for I3. A page read for
  the first time in a transaction is checked.
- **Risk:** item 6, as in O6.

### O8 — Journal the changed bytes of each page

- **Kind:** changes the journal format and its recovery ([journal.prd.md](../prd/krummi/journal.prd.md): J16 and the
  record format) → PRD first.
- **What:** as Voron does, journal each changed page as its difference from its version before the transaction:
  runs of changed 8-byte words, then compressed. A new page is journaled as its non-zero runs. Recovery applies the
  differences in order over the data file page.
- **Targets:** W2, W14, W15 and W19 (LZ4 36–40 %) and W21 (23 %). It also cuts the bytes every write load writes.
- **Invariants to keep:**
  - Recovery restores every committed transaction and nothing else, as today (journal PRD).
  - A torn data file page is still restored. The PRD needs the proof: each difference applies over a page whose
    earlier versions are in the journal or in the data file.
- **Risk:** item 1 (the stored format) and item 3 (the journal format). It is the largest of the four.

## Order

1. O1: it removes the largest cost under concurrency and needs no product decision.
2. O2: same kind, smaller.
3. O3a: small and independent; every load gains.
4. O3: write its PRD while O1 and O2 run; it is the largest cost at one thread.
5. O4, then a new diagnosis of the writes on Windows, where W4 still misses.
6. O5: it needs no product decision, and every commit-bound write load gains. Run it while the PRD amendments
   of O6 and O7 are written; one amendment can cover both, since both are about what the writer checks.
7. O6: it removes the largest single cost of W1 and W3.
8. O7.
9. O8: the largest lever for the random-key loads, and the largest change. Write its PRD after O5–O7 have measured
   what is left.

Each step closes with complete runs on Windows and Linux, so the next diagnosis starts from measured numbers.

## History

| Date | Optimization | Commit | Result |
|---|---|---|---|
| 2026-11-04 | Diagnosis of reads and writes | — | Profiles of W7, W9, W22, W2 and W21 on WSL 2 (this document) |
| 2026-11-04 | O1, pager mapping without a lock | `9acf695` | Single-load runs, before → after: W9 +31 % on Windows and WSL 2, W10 +49 % / +116 %, W8 0 % / +21 %; no threshold met yet ([report](../../odd/tasks/pager-mapping-report.md)) |
| 2026-11-04 | O2, scratch lookup without a lock | `387e758` | Single-load runs, before → after: W9 +26 % to +66 % on Windows, +63 % to +131 % on WSL 2; the other loads within noise; a load after W9 can run slowly on Windows under the same parent process ([report](../../odd/tasks/scratch-lock-report.md)) |
| 2026-11-05 | O3a, xxh3 picked at run time | `8c80d79` | Single-load runs, before → after: W7 +21 % to +57 %, W8 +15 % to +31 %, W9 +25 % to +52 %, W11 +7 % to +18 %, W12 +3 % to +15 % on Windows and WSL 2; W10 and W13 within noise; checksum values pinned, with the SSE2 and scalar variants checked in CI; no threshold met yet ([report](../../odd/tasks/fast-xxh3-report.md)) |
| 2026-11-05 | Complete runs after O1, O2 and O3a | `3ec8641` | Against the 2026-11-03 reports, Krummi's op/s: every read load rises on both platforms (Windows +14 % to +208 %, Linux +6 % to +128 %; W7 +29 % / +37 %, W9 +208 % / +128 %); writes go from −2 % to +34 % on Windows and from −17 % to +7 % on Linux, where Voron's W1 and W4 fall too. Thresholds met: Windows 6 of 22 (was 5, adds W4), Linux 5 of 22 (was 7, loses W1 and W20). By the 0.05 ratio rule, Windows W5, W6 and W16 and Linux W1, W10, W16 and W20 count as worse, each with Krummi flat or up and Voron up, except Linux W1 (Krummi −17 %, Voron −8 %) and W16 (Krummi −12 %, still 74 times Voron); the unchanged engines moved between the two dates too (Voron −37 % to +50 %, canopydb and fjall up to several times), so the rule is too tight to tell noise and needs calibrating. A first Windows run on 2026-11-05, with other work on the machine, was discarded: the unchanged engines moved −86 % to +445 % ([Windows](../benchmark/2026-11-05-windows.md), [Linux](../benchmark/2026-11-05-linux.md)) |
| 2026-11-06 | O3, remembered page integrity | `13489d1` | Single-load runs, before → after: W7 +87 % to +126 % on Windows, +104 % to +113 % on WSL 2; W9 +231 % / +163 % to +175 %; W22 +245 % to +261 % / +104 % to +220 %. Codex R1 (a run's length mark can be evicted) left as a user decision ([report](../../odd/tasks/integrity-once-report.md)) |
| 2026-11-06 | O4, fewer zeroed buffers and allocations in the commit | `d95da27` | Single-load runs, before → after: W21 +17 % to +20 % and W14 +9 % to +10 % on Windows, W2 and W4 +3 % to +9 %; on WSL 2 every write load within noise; journal record bytes pinned ([execution](../../odd/tasks/commit-copies.md)) |
| 2026-11-06 | Complete runs after O3 and O4 | `2fdf8e9` | Thresholds met: Windows 10 of 22 (was 6, adds W7, W8, W9 and W22), Linux 10 of 22 (was 5, adds W7, W8, W9, W20 and W22). Ratio Krummi / Voron, W7: 0.50 → 1.23 on Windows, 0.54 → 1.03 on Linux; W22: 0.52 → 1.27, 0.38 → 1.21. By the 0.05 rule, Windows W10, W11 and W20 and Linux W2, W4–W6, W14–W17 count as worse; Krummi's own op/s fall only in Windows W10 (−30 %) and Linux W2, W4–W6, W14–W17 (−4 % to −20 %), and single-load runs against `3ec8641` (before O3) show none of them lower: W10 +5 % / +10 % on Windows; W2, W14, W15 and W17 +0 % to +9 % on WSL 2; W16 +1 % and −32 % with reps from 254 to 606 ([Windows](../benchmark/2026-11-06-windows.md), [Linux](../benchmark/2026-11-06-linux.md)) |
