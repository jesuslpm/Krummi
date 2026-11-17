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
| T2 | S2 | delegated writer (parallel) | done | `0a5603c`; L2, L4 |
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
