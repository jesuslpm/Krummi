# Write optimizations: remembered checks of written pages, journal compression, range step, recent leaves, loop alignment

Objective: the six optimizations left after the write-margin run. Branch `feat/write-optimizations` from
`feat/write-margin` (`10077aa`, unmerged; the PRD commit `10bba76` is the first of the run). Delivery: one commit per
task on the branch, no merge or push. Tests: `cargo test --workspace`; acceptance with Krummi-only A/Bs and complete
`--solo-krummi-voron` runs on Windows and WSL 2. The user asked to finish without asking: every decision is taken by
the agent and listed in the closing report.

## Specs

- **S1** — The PRD [pager-written-pages.prd.md](../../docs/prd/krummi/pager/pager-written-pages.prd.md) (Spanish,
  authoritative), decisions Q1–Q7 and criteria AC-Q1–AC-Q10, with its annotations to the ancestry. Quoted, Q1: «Las
  páginas que I4 deja de recordar antes de que el traslado las escriba (…), salvo las de un valor fuera de su hoja,
  quedan recordadas como comprobadas, con ese código, en cuanto sus bytes quedan escritos en su sitio»; Q3: «La
  versión que publica la confirmación de una hoja que cumple Q3 conserva la comprobación de organización»; Q4: «En las
  compilaciones de depuración, toda lectura por otra transacción de una hoja que conserva su comprobación por Q3 (…)
  comprueba además su organización, fuera de las comprobaciones que cuentan los criterios».
- **S2** — No compression when it does not help (W17 `valores-grandes-carga`): the journal stops spending time
  compressing data that does not get smaller, within J16 of the journal PRD («los datos que no se comprimen se guardan
  igualmente, sin error. La compresión no cambia nada de lo que leen las transacciones ni de lo que contiene
  `krummi.data`»); the algorithm is left to the implementation. No change of behavior or of what recovery accepts.
- **S3** — The step between entries of a range (W11, W27): look for a measured gain in the per-entry step of a range
  scan; keep a change only if an A/B shows it. No change of behavior.
- **S4** — A cache of recent leaves, as Voron's recently found pages, for writes and reads whose keys come close
  together (W19, W1, W23): keep only if an A/B shows a gain and nothing worse. No change of behavior or stored bytes.
- **S5** — Loop alignment in the release build (the W20 gap of the write-margin run): keep only if an A/B over all
  loads shows no load worse and W20 back. A build setting: Risk item 5.
- **S6** — Acceptance: per-change Krummi-only A/Bs (at least ten rounds for loads under one second per repetition or
  with several threads, three for the others), complete runs on Windows and WSL 2 under the bench power plan, the
  closing report with every measured figure.

Out of scope: anything the PRD's «Fuera de alcance» lists; R2 of the write-margin code review (pager writer GC).

## Tasks

| ID | Specs | Route | Status | Commit / evidence |
|---|---|---|---|---|
| T1 | S1 (Q1–Q7) | delegated writer (context backstop; parallel with T2, T3) | done | `56f8a79`, `757901b`, `0c8bea7`; L2–L4 |
| T2 | S2 | delegated writer (parallel) | reverted (L11) | `0a5603c`; revert `f45269d` |
| T3 | S3 | delegated writer (parallel) | done | `ce8a668`; L2, L3 |
| T4 | S4 | delegated writer after T1 | reverted (L6) | `3f35631`, `49fc00d`, `197d41c`, `3c11fba`; revert `8b54d0b` |
| T5 | S5 | inline, at the end | not adopted (L7) | measurement only |
| T6 | S6 | inline, background runs | done | L6–L9 |
| T7 | — | close: RDD of the last slice, Codex review, report, archive | pending | |

Route evidence: the parent context is past the long-session backstop (this session carries two earlier runs), and
T1, T2 and T3 are independent units with disjoint edit surfaces, each several files or 100+ lines.

## Log

- **L1** (2026-11-16) — The user's requests, verbatim: «Hagamos todas las optimizaciones»; to the estimate (8–12 h):
  «Sí, termina la ejecución sin preguntar, toma tú las decisiones». Start 2026-11-16 18:38:05 +0000. The PRD of S1
  was written and approved earlier in the session (decisions «Sí, como Voron», «Sí, solo valor igual», «Sí, se
  encadena»; Codex PRD review approved in round 2). The untracked `docs/prd/krummi/tables/tables-indexes.prd.md` is
  not part of the run and is never staged.
- **L2** (2026-11-16) — Writers, launched together (T1, T2, T3). T2 `0a5603c`: a commit record of 4 MiB or more is
  probe-compressed on eight 32 KiB samples from the middle of equal parts and stored raw when they shrink by less
  than 1/32; records under 4 MiB unchanged; RED `assertion failed: !header.compressed`, then GREEN; raw records were
  already valid for recovery. T3 `ce8a668`: each end of a range keeps its leaf and index apart from the path and
  steps through a clear leaf with one comparison; `node::entry` inlined across crates; behavior-preserving, no RED;
  writer's micro-measurement: whole scans 13.8–15.1 → 8.3–11.3 ns per entry, W11-like 39.4–40.6 → 33.8–35.4 (noisy,
  other builds running). T1 `56f8a79`: Q1–Q4 of the PRD (the transfer remembers what it writes; a
  `keeps_check` mark kept only by same-length overwrites through `page_mut_same_layout`; debug re-check), tests
  AC-Q1–AC-Q9 in `crates/krummi/src/written_pages_tests.rs`, RED observed for each failing one; existing tests
  adjusted as the PRD annotations allow, plus AC-I9 and AC-S5 (temporary part), whose annotations were added in
  `757901b`. Seam check: fmt and clippy clean, `cargo test --workspace` 867 passed, 0 failed, 35 ignored.
- **L3** (2026-11-16) — RDD. The whole run (`10077aa..757901b`, 2,839 lines) exceeded the reviewers' context budget
  (`lens_context_budget_exceeded`, no authority created), so it was split: T1 alone (`ce8a668..757901b`, lenses
  risk, resilience and reliability, item 1 named), lineage `review-8e27299c4cffef65`, approved and acknowledged;
  T2 and T3 (`aa82944..ce8a668`), reviewed in a local clone at `ce8a668` (`work/rdd-clone`, since the review always
  ends at HEAD), same lenses, lineage `review-a2930b6b3ea7e376`, approved and acknowledged. T1 findings: R4-001
  (SUGGESTION, the order of the staged pages is only a comment): a debug assertion added; two WARNINGs dismissed with
  evidence: the value-run mark is set by the only path that stores runs in the scratch area
  (`scratch.rs:1526-1527`; recovery does not use it), and `copy_for_write` reading through `page_and_code` is what
  `Snapshot::page` already does (`scratch.rs:1202-1204`). T2+T3 findings: three WARNINGs on the derived last key of a
  range end after a step into a new leaf that returns nothing, dismissed with evidence: `guard` stops both ends on
  every `Ok(None)` and error (`range.rs:495-506`, `stop` at `:253-258`), so no later step reads it; one SUGGESTION
  (a unit test alternating ends): covered by the random alternation tests of `crates/krummi/tests/tree_scans.rs` and
  `tree_value_model.rs`.
- **L4** (2026-11-16) — Independent verifier of T1 and T2 (sonnet, high): S1 pass, S2 pass, no blocker. It checked
  every path that writes a dirty page clears the Q3 mark (only `page_mut_same_layout` keeps it, three callers), Q1
  remembers only after the write and never a value-run page, and recovered each build's journal with the other build
  (`aa82944` and `0a5603c`, four pairings, 408 of 408 values). Advisories: an unused `PoisonError` import in release
  builds (fixed in `0c8bea7`, with the R4-001 assertion); a large record whose compressible bytes miss the eight
  samples is stored raw (+4.7 % in one probe, allowed by J16); the name of the AC-U6 test no longer matches its
  assertion (kept: archived reports cite it); debug reads of kept leaves take a mutex.
- **L5** (2026-11-16) — T4 `3f35631` (writer, after T1): a write transaction remembers the last leaf its walk
  reached (root, page, copies of its bounding keys); `set`, `try_add` and `remove` change it directly when the change
  stays in the leaf; forgotten on every page reservation, free and failure. Writer's measurement: W19 keys are hashed
  (0.15 % same-leaf), so no cache helps it; W23 already uses `append_last`; W1-like sets −21 to −28 % per set. Tests
  compare every page with the cache on and off; mutations of the forgets caught. Suite: 872 passed, 0 failed, 36
  ignored; clippy clean in debug and release. RDD `757901b..3f35631`, lenses risk, resilience and reliability, item
  6 named, lineage `review-6e9928eb4aa5471f`: approved; R1-001 (WARNING, `remember_leaf` public): hidden from the
  docs in `49fc00d` (it cannot be crate-private: another crate calls it); R4-001 (WARNING, a miss that splits walks
  twice): left to the A/B; R3-001 (WARNING, the correctness rule is only documented): checked by the verifier;
  R3-002 (SUGGESTION, a walk-count threshold in a test): kept. Verifier of T4 (sonnet, high): S4 pass, no blocker;
  every shape change reserves or frees a page (no redistribution between siblings; root collapse frees); a debug
  cross-check of each cache hit against a walk held over about 980,000 hits in random workloads and fired within a
  second when a forget was removed; 84 workload digests identical to `0c8bea7`. Its advisory, a stale hit inside a
  transaction that already failed (the tree is already inconsistent there at the baseline too), was hardened in
  `49fc00d`: a failed transaction skips the cache. Tree, tx and krummi tests after it: 626 passed, 0 failed.
- **L6** (2026-11-16) — T6 A/B, the write-margin head (`cur`, same code as `1ee31df`) against this run's head, worker
  on the performance cores, bench power plan, digests equal (`work/wm/ab.py`; `opt-a.json`, `opt-b.json`, `opt-c.json`).
  With T1–T4 (`49fc00d`), 3 rounds: W11 1.162, W1 1.130, W27 1.082, W28 1.077, W26 1.045, W17 1.042, W23 1.008, W21
  1.005, W24 0.995, W2 0.992, W19 0.991, **W3 0.974 (slower in 3 of 3)**. A back-off of the leaf cache (`197d41c`)
  made W3 worse (0.947). Bisection, 5 rounds (`bis.json`), W3 ns per insert: base 2423, after T2 2452, after T1 2354,
  after T4 2497, with the back-off 2528: T4 costs W3 about 6 %; W9 (8 rounds) spreads 190–242 ns in every build:
  noise. The T4 writer restructured the cache (`3c11fba`: a miss takes the usual single walk, which remembers the
  leaf; only a hit goes to it): its micro-measurement put W3 within noise, but the bench, 5 rounds (`t4b.json`), still
  gave W3 2486 against 2363 before T4 (slower in 5 of 5) and 2443 at the base, while W1 gained 9 % on the base (12 %
  on the head before T4). S4 keeps the cache only if nothing gets worse: T4 was reverted (`8b54d0b`, reverting
  `3f35631`, `49fc00d`, `197d41c`, `3c11fba`); the code is the same as at `0c8bea7`. Decision taken during the run,
  listed in the closing report: a cache limited to ordered writes may keep W1's gain; not tried.
- **L7** (2026-11-17) — T5 (S5), loop alignment. The head code (same as `0c8bea7`) built with and without
  `-C llvm-args=-align-loops=64`, A/B on every load but W16 (`al-a.json` 3 rounds, `al-b.json` 10 rounds for the short
  and multithreaded loads): W20 0.998 (slower in 6 of 10), so W20 does not come back; W6 0.964, W18 0.973, and W2, W4,
  W21 about 0.994 slower in 3 of 3; the rest within ±2.5 %. S5 keeps it only if W20 comes back and nothing gets
  worse: not adopted, nothing committed. Against the write-margin head, this run's W20 already measures 1.014
  (`opt-b.json`, 10 rounds).
- **L8** (2026-11-17) — T6, complete runs with `--solo-krummi-voron` on the final code (`8b54d0b`, same code as
  `0c8bea7`). Windows, performance cores, bench power plan: **29 of 29**
  (`docs/benchmark/2026-11-17-windows-write-optimizations.md`). Against the write-margin run of 2026-11-16, ratio to
  Voron: tabla-actualizacion (W28) 0.88 → 0.95 (Krummi +14 %), tabla-lecturas 0.90 → 0.99, intervalos 1.08 → 1.23,
  recorrido 2.47 → 3.00, carga-en-orden 1.33 → 1.42, tamano-fijo-carga 0.96 → 1.00; the concurrent reads that fell in
  that run are back (W8 1.21, W9 1.44, W12 1.70, W13 1.72), as its A/B had shown. The first WSL run stopped at once:
  the previous report was still in `~/wm` and the bench does not overwrite one; the script now removes it first.
- **L9** (2026-11-17) — WSL 2, complete run under the bench power plan, final code (00:54–01:39): **29 of 29**
  (`docs/benchmark/2026-11-17-linux-write-optimizations.md`). WSL moves a lot between days (recorrido 2.51 → 1.51,
  apertura 106.81 → 177.74 with the same engines' code for Voron); the ratios that fell more than 0.05 against
  2026-11-16 were checked with an A/B in WSL, the write-margin head against this run's head, 5 rounds
  (`work/wm/linux/ab2.txt`): W10 1.141, W28 1.064, W24 1.007, W23 0.988 (slower in 5 of 5); a bisection of W23, 6
  rounds (`work/wm/linux/bis.txt`): base 7262, after T2 7206, head 7260 ns per row: no drop. T6 done.
- **L10** (2026-11-17) — RDD of the last slice `3f35631..e0e4fbe` (the T4 revert and the records), lenses
  reliability and readability, lineage `review-d6eb8d83236a06a9`: approved and acknowledged. Its findings on the
  identity of the measured code, answered here: the Windows report's «con cambios sin confirmar» were this execution
  document and its phase log only, no code (R2-002); the WSL report's «Krummi: desconocido» is because the WSL worker
  is built from `git archive` of `8b54d0b` (`work/wm/linux/src.tar`, made at that commit) and carries no `.git`, as
  in the earlier Linux reports (R2-001). `8b54d0b` has the same code as `0c8bea7` (`git diff 0c8bea7 8b54d0b --
  crates` is empty).
- **L11** (2026-11-17) — Codex code review, round 1, of `10077aa..2417fd9`
  ([report](../../docs/reviews/write-optimizations/2026-11-17-code.md),
  [resolutions](../../docs/reviews/write-optimizations/2026-11-17-code-resolutions.md)): `findings`, R1–R3 major, R4
  minor. R1 (the T2 samples miss a repetition longer than 32 KiB: a value run that compresses 31× was stored raw,
  against Y1 and J16): T2 reverted in `f45269d`; no sampling rule keeps Y1 for every record, so W17's gain is given
  up. R2 (a new `Vec<bool>` of keep-check flags allocated during commit, against Q6) and R4 (a debug mutex on every
  C2 hit, against C6): fixed in `cf8fd80` by a writer: one `PageChange` per page in place of the created flags and
  the keep-check flags (the commit reserves what it did before), and a debug-only array of atomics beside the checked
  table in place of the map. Suite 867 passed, 0 failed, 35 ignored. R3 (the A/B matrix of Q7 incomplete): the final
  code (`cf8fd80`) against the write-margin head, Windows (`q7-a.json` 3 rounds, `q7-b.json` 10 rounds): W28 1.075,
  W27 1.053, W12 1.028, W3 1.025, W17 1.022, W7 1.021, W26 1.020, W22 1.016, the rest 0.995–1.005; W23 0.978 (3 of
  3) and W21 0.997 (3 of 3) measured again in 10 rounds (`q7-c.json`): W21 1.001 (same), **W23 0.998, slower in 8 of
  10**: by the letter of Q7 a worsening of 0.2 %, under the spread of one run; Q7 leaves it to the user, listed in
  the closing report. T2 is now not part of the run: its row of the task table reads reverted.
- **L12** (2026-11-17) — RDD of the last slice `e0e4fbe..5bbeb4e` (the T2 revert `f45269d`, the fixes `cf8fd80` and
  the records), lenses risk, resilience and reliability, lineage `review-56d68a6bfbe776dd`: approved and acknowledged
  (authority burned). Risk and resilience found nothing. Reliability left two non-blocking advisories, kept as later
  work: R3-001 (WARNING, inferential): the debug record of kept checks and the slot are two relaxed stores, ordered
  for a reader only by the scratch area's lock, which no test exercises; it only touches the debug builds' extra
  check, never what a read returns. R3-002 (SUGGESTION): `PageChange::new(true, true)` gives `Created` and drops the
  keep-check flag; created pages are never marked, and a debug assertion would contradict the test that pins the
  drop (`scratch_tests.rs:390`), so it stays as it is.
- **L13** (2026-11-17) — R3, the Q7 matrix in WSL 2: the final code (`cf8fd80`, built from `git archive` in
  `work/wm/linux/src.tar`) against the write-margin head (`10077aa`), bench power plan (`work/wm/linux/q7.sh`;
  `q7-a.txt` 3 rounds, `q7-b.txt` 10 rounds), speed ratio: W27 1.166, W23 1.085, W3 1.071, W28 1.071, W17 1.036, W19
  1.026, W22 1.021, W7 1.013, W26 1.012, W1 1.010, W2 1.009, W29 1.007, W6 1.004, W4 0.995; slower: W5 0.991 (9 of
  10), W12 0.976 (7 of 10), W14 0.983, W15 0.975, W21 0.962, W24 0.974, W25 0.972. Measured again
  (`work/wm/linux/q7r.sh`, 06:13–08:38; `q7r-a.txt` 20 rounds, `q7r-b.txt` 6 rounds), median of the rounds: W5 1.023
  (slower in 7 of 20), W12 0.995 (11 of 20), W14 1.065 (2 of 6), W15 1.002 (3 of 6), W21 0.998 (3 of 6), W24 0.993 (3
  of 6), W25 1.008 (3 of 6): every one slower in about half its rounds, so none is a worsening; the first figures were
  WSL's spread over too few rounds. R3 complete: no load gets worse on Linux; on Windows only W23's 0.2 % (L11).
- **L14** (2026-11-17) — Codex code review, round 2, of `2417fd9..df8abb1`
  ([report](../../docs/reviews/write-optimizations/2026-11-17-code-02.md), resolutions as in L11): `findings`. R1, R2
  and R4 verified resolved. R5 (major, new, from R4's fix): a read that finds a kept leaf in its slot loses its record
  when the transfer writes another page of the same slot before the read asks, and the debug check of Q4 is skipped;
  fixed in `b025d58` (the record is not trusted once the slot changed; test
  `a_kept_content_replaced_after_its_hit_is_still_checked`, RED then GREEN; suite 868 passed, 0 failed, 35 ignored;
  release builds unchanged). R3 (reduced to minor): the figures of L11 and L13 are corrected here, by Q7's rule (median
  after below before, and below in most rounds), the median of each round's repetitions, W12 and W13 apart (`ab2.sh`
  labelled both W12). Windows: W6 0.998 (6 of 10), W19 0.995 (2 of 3) and W23 0.998 (8 of 10) get worse; measured
  again in 20 rounds (`work/wm/q7-d.json`, the builds of L11, `fin` from `cf8fd80`, whose release code `b025d58` keeps; 09:04–09:34 by
  the phase log): **W6 0.997, slower in 11 of 20: a worsening of 0.3 %**, W19
  1.001 (10 of 20) and W23 1.005 (13 of 20) do not. WSL, first matrix: W12 1.028 (1 of 10), W13 0.956 (8 of 10), W14
  0.983, W15 0.975, W21 0.962, W24 0.974, W25 0.972, W5 0.991 get worse; measured again: W5 1.014 (7 of 20), W12 1.036
  (2 of 20), W13 1.001 (12 of 20), W14 1.005 (2 of 6), W15 1.000 (3 of 6), W21 1.001 (3 of 6), W24 0.990 (3 of 6), W25
  0.998 (3 of 6): none does. What still gets worse by the letter of Q7 is W6 on Windows, 0.3 %, for the user.
- **L15** (2026-11-17) — RDD of the slice `5bbeb4e..6dc29e0` (R5's fix `b025d58` and the records), lenses
  reliability and resilience, lineage `review-c10d9a58577cf96f`: approved and acknowledged (authority burned).
  Resilience found nothing. Reliability left three non-blocking advisories: R3-001 (WARNING): the fix holds across
  threads by its store and load order, which the one-thread test does not exercise (as L12's R3-001), kept as later
  work; R3-002 (SUGGESTION): `layout_kept` answers yes for any content its slot does not hold, so it only means
  something after a hit, as its documentation says («A read asks it after finding the content in its slot»); R3-003
  (SUGGESTION): L14's time and measured code, corrected there from the phase log.
- **L16** (2026-11-17) — Codex code review, round 3, of `df8abb1..8bff765`
  ([report](../../docs/reviews/write-optimizations/2026-11-17-code-03.md), resolutions as in L11): `approved`. R5 and
  R3 verified resolved; R1, R2 and R4 had been in round 2. Its one open question is Q7's decision for the user: W6 on
  Windows, 0.33 % slower in the 20-round measurement; it goes to the closing report, to be asked after the run.
- **L17** (2026-11-17) — T6, complete `--solo-krummi-voron` runs on the final code (`886db96`; its code is
  `b025d58`'s; «con cambios sin confirmar» is the phase log only; WSL built from `git archive` of `886db96`), bench
  power plan. First run (09:42–10:50): Windows **27 of 29**, missing `lecturas-con-escritor` 0.73 and
  `escritor-con-lectores` 0.79; WSL **28 of 29**, missing `lecturas-al-azar-16` 0.68
  (`docs/benchmark/2026-11-17-{windows,linux}-write-optimizations-final-1.md`). Voron moved as much (Windows
  `lecturas-con-escritor` 1.67 M → 2.32 M op/s; WSL `lecturas-al-azar-16` 4.10 M → 3.13 M), and Q7's A/Bs, interleaved
  over 10–20 rounds, give these loads no worsening (L14), so the run was repeated unchanged. Second run
  (10:52–11:59): **29 of 29** on both (`…-final-2.md`): Windows W9 1.15, W12 0.97, W13 0.96; WSL W9 1.17, W12 1.19,
  W13 1.08. Against the `8b54d0b` runs (L8, L9), Krummi's `valores-grandes-carga` (W17) on Windows fell 6,563 → 5,617
  → 4,696 op/s while Voron stayed at 5,500–5,700; an A/B of W17 on Windows, 10 rounds, the write-margin head against
  the final code (`work/wm/q7-w17.json`): **1.010, slower in 4 of 10**: no worsening; the runs' figure is the
  machine's state, and the T2 gain was given up (L11). T6 done.
