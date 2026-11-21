# JSON columns of the tables — ODD execution document

Objective: implement the JSON columns of `docs/prd/krummi/tables/tables-json.prd.md` (7.5, approved by the Codex PRD
review on 2026-11-21). Branch: `feat/tables-json` (base `cf73c36`; `main` holds the PRD at `f61a259`). Delivery:
`ask-on-risk` (single branch, left unmerged). Test runner: `cargo test --workspace` (Windows; Linux in WSL 2 Ubuntu).
Estimate: 14 h – 22 h. Real start: 2026-11-21 08:01:01 +00:00.

## Specs

- **S1** — The JSON columns PRD, `docs/prd/krummi/tables/tables-json.prd.md`, and its ancestry up to the root:
  decisions JS1–JS14, *Validaciones y mensajes*, *Salida*, *Fuera de alcance* and *Dejado a la implementación*,
  verbatim there. Its acceptance criteria AC-JS1–AC-JS13 are the tests of the library; AC-JS11 (9) is the fuzzing run.
- **S2** — JS13 and AC-JS14: one-round pass of W1–W32 before and after, 10 to 20 rounds of W23–W29 and of any load
  that moves more than 3 %, and the informative measurement of JSON documents of about 200 B, 4 KB and 64 KB against a
  `string` floor, on Windows and Linux.
- **S3** — The technical starting state: `docs/design/tables/13-json-column-storage.md` (*Integration with the row
  format*, *Recommended design*: stored form v1 with its value header, invariants I1–I8, *Tests and acceptance
  criteria for PRD 7.5*).
- **S4** — The project's checks (`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`), RDD on each work-unit commit, the Codex code review over the whole range, and the closing
  report.
- **S5** — The user's instruction for the run (L1): «Implementa el PRD 7.5 con ODD, sólo pregunta al principio, luego
  no preguntes nada toma la decisión recomendada.»

## Execution design

- **Errors.** `ValueRule` gains `InvalidJson`, `MalformedJsonPointer` and `InvalidReadScale` right after
  `MalformedText` (E17 order); `SchemaRule` gains `JsonColumn` next to `VectorColumn`. `Error::InvalidValue` gains an
  optional boxed `JsonFault { cause, position, pointer }` (cause `InvalidUtf8 | Syntax | LoneSurrogate |
  RepeatedProperty(name) | TooDeep`), so the error stays within its size test.
- **Column and value.** `ColumnType::Json`, type code 21, variable section, not orderable, no statistics.
  `Value::Json(Json<'a>)`: the text given (unvalidated) or the normalized text read back. `normalize` turns a JSON
  value into its normalized text or fails with `InvalidJson`; the write path computes the `JsonFault` detail only on
  that failure. Stored form v1: header byte `0x01` (form 1, no index), then the normalized text.
- **Reader** (`table/json/`): one pass over the input bytes, explicit stack bounded by depth 128, duplicate check per
  object by sorting the normalized names (pairwise under 8 members), the smallest-position fault rule of JS8 (on a
  later fault, duplicates already present in the open objects are considered). No dependency, no `unsafe`.
- **Navigation and typed reads** (`table/json/`): `JsonValue<'a>`, a view over normalized text: kind, text, members,
  elements, lookup, RFC 6901 pointer; typed reads with exact decimal arithmetic on the number text (no exponent
  expansion), `f64` by `str::parse`, date and RFC 3339 instant parsing.
- **Reads.** A typed accessor and a projection marker return `JsonValue<'t>` borrowed from the row or the index entry;
  a copied row's value navigates its own copy.
- **Test-only dependency:** `serde_json` (default features) as a dev-dependency of `krummi`. JSONTestSuite vendored
  under `crates/krummi/tests/data/`. Fuzz targets in a `fuzz/` crate outside the workspace, run in WSL.

## Tasks

| ID | Specs | Route | Status | Commit |
|---|---|---|---|---|
| T1 | S1: JS8, JS10 (error rules and the JSON fault data); S3 (module skeleton and API) | inline | done (no RED: types and stubs only; build, clippy, pager and lib tests pass) | `f5f7e42` |
| T2 | S1: JS2, JS4, JS8, JS14 (reader); AC-JS2, AC-JS3 (reader level), AC-JS11 (1, 3, 5, 6, 7, 8), reader fuzz target | delegated: parallel with T3 (Writer trigger a) | done (RED: 18 of 19 reader tests failed on the stub; 20 tests pass; 20 hand mutants killed; fuzz target moved to T5) | `28b9363` |
| T3 | S1: JS6, JS7 (navigation, typed reads); AC-JS5, AC-JS6, AC-JS11 (2, 4), navigation fuzz targets | delegated: parallel with T2 (Writer trigger a) | done (RED: 28 of 28 unit tests failed on the stubs; 28 pass; fuzz targets moved to T5) | `cbd7978` |
| T4 | S1: JS1, JS3, JS5, JS9, JS10, JS12 (tables integration); AC-JS1, AC-JS3, AC-JS4, AC-JS7–AC-JS10, AC-JS12, AC-JS13 | delegated (context backstop) | done (RED: the new test binary did not compile, 37 errors; AC-JS12 failed before the format bump; 14 tests pass) | `d86eb6e` |
| T5 | S2 (bench hook and measurements), AC-JS11 (9) fuzzing run | inline | done (fuzzing 1 h per target, no failure; JS13 measured on Windows and Linux, report *Mediciones*) | `e834b10`, `cfe78a5`, `94a5150` |
| T6 | S4 (reviews, closing report, archive) | inline | done (verifier, Codex, RDD of the last slice; closing report) | `40e2d92`, report commit |

## Log

- **L1** (2026-11-21, user): «Implementa el PRD 7.5 con ODD, sólo pregunta al principio, luego no preguntes nada toma
  la decisión recomendada.» No product question was open after the code map; estimate 14 h – 22 h, confirmed: «Sí,
  empieza».
- **L2** (2026-11-21): the branch holds two design commits from another session (`050ad5c`, `cf73c36`), docs only;
  the run's base is `cf73c36`.
- **L3** (2026-11-21): T1 committed (`f5f7e42`). RDD assess against `cf73c36` (untracked execution files excluded):
  medium, `under_budget`, pending in the slice. The column type, value and format 14 moved to T4, which needs the
  reader and the navigation. Known environmental failure: `ac_p6_concurrent_openers_of_a_missing_folder_get_one_store`
  failed once in a full pager run and passed 5 of 5 alone (intermittent under load, untouched code).
- **L4** (2026-11-21): the Engram mirror `odd/tables-json/tasks` is pending: auto mode could not review `mem_save`
  because a hook changes its input. This file is the only copy.
- **L5** (2026-11-21): JSONTestSuite vendored at `crates/krummi/tests/data/json_test_suite` (commit `1ef36fa`, MIT),
  marked binary in `.gitattributes` so its bytes stay exact. T2 and T3 launched in parallel (Writer trigger a:
  independent units, disjoint surfaces).
- **L6** (2026-11-21): T2 (`28b9363`) and T3 (`cbd7978`) committed after the seam check: `cargo fmt --all --check`
  clean, clippy clean, `cargo test --workspace` 1,081 passed, 0 failed. RDD assess of `cf73c36..HEAD`: medium,
  `under_budget`, pending. The other session keeps committing design document 15 on this branch (`b9196a0`, `5d7d588`,
  `59190d5`, docs only); they stay in the range and the closing report keeps them apart. Writers' assumptions kept
  (most faithful reading of JS7 and JS8): a high surrogate not followed by `\u` and a low surrogate is lone, ahead of a
  later syntax fault; at one position invalid UTF-8 beats a text that ends early; an instant's fraction is rounded
  from its first nine digits (same result); a pointer step too large for `usize` is absent.
- **L7** (2026-11-21): T4 committed (`d86eb6e`) after `cargo fmt --all --check`, clippy and `cargo test --workspace
  --no-fail-fast` (1,095 passed, 0 failed). Decisions of the writer kept: the typed accessor and the projection marker
  (`JsonDocument`) return `Json<'t>` (a copied row must own its document, PY3), navigated with `document()`; E5 and
  IX3 reuse `InvalidKeyColumn` and `VectorIndexKey`, whose texts now name JSON; a change to or from `json` is
  `UnsupportedChange`. Format pins outside the listed surfaces were updated (krummi-tx `recovery.rs` and
  `bytes_tests.rs` hash, krummi-pager `pages.rs`). Risk: items 1 (format 14, stored form v1) and 3 (key, index and
  statistics checks shared with vectors) → independent verifier launched.
- **L8** (2026-11-21): T5 so far: fuzz targets (`cfe78a5`, build fix `fixed after a failed first build in WSL`),
  nightly and cargo-fuzz 0.13.2 installed in WSL; the informative measurement `json_measure` (`e834b10`); measurement
  scripts in `work/tj/` (git-ignored, from `work/st/`), to run after the verifier's verdict.
- **L9** (2026-11-21): independent verifier (sonnet, over the whole document, baseline `cf73c36`): no blockers; JS1–JS12
  and AC-JS1–AC-JS13 met, AC-JS11 (9) and JS13/AC-JS14 pending in T5; a non-JSON program gives identical output on the
  baseline and HEAD except the Display texts of `InvalidKeyColumn` and `VectorIndexKey` (intended: their rules now
  cover JSON). Advisory fixed: `Json` equality and hashing compared the internal normalized flag, so a read-back
  document did not equal `Json::new` with the same text; now by bytes (`e302ea9`, test
  `values_with_the_same_bytes_are_equal_whether_given_or_read_back`, red before the fix).
- **L10** (2026-11-21): RDD. Assess of `cf73c36..HEAD` with `--escalate-item 1` (format 14, stored form v1): high,
  `review_due`. The returned preflight STATUS with lenses risk, resilience, readability, reliability failed with
  `gentle-ai.review-integration.failure/v2` `operation_timeout` (phase `pre_native`, mutation `not_started`,
  `retry_safe: false`, `manual_action_required`). Recorded as unavailable; not retried blindly. The closing slice
  review (CLAUDE.md) will run a fresh preflight once the measurements end; the possible Gentle AI defect report is
  left for the user after the run (L1: no questions during it).
- **L11** (2026-11-21): fuzzing. `json_normalize`: 13,314,383 runs in 3,601 s, no failure and no artifact. The first
  launch of both targets lost its shell variables through `wsl.exe` and `json_navigate` never started; it runs again
  alone with `work/tj/fuzz.sh`.
- **L12** (2026-11-21): RDD, second attempt. A fresh preflight of `cf73c36..HEAD` timed out again (`operation_timeout`,
  26 s, idle machine). Reviewing the high-risk unit alone (`--base-ref cbd7978`, with `--escalate-item 1`): the
  preflight first required a decision on the untracked execution files, so they were committed (`ba15c45`); then
  START returned the consent envelope, consent granted per CLAUDE.md («Automated review consent is always granted»),
  and the granted START failed with `lens_context_budget_exceeded` (no authority created, nothing to abandon). The
  range holds 2,136 changed lines of this run plus about 900 of the other session's design document 15; splitting it
  would rewrite a branch another session commits to, so the native review of T4 is recorded as unavailable. Covered by
  the independent verifier (L9) and the Codex code review of the whole range.
- **L13** (2026-11-21): Codex code review of `cf73c36..HEAD`. Round 1 (`docs/reviews/tables-json/2026-11-21-code.md`):
  one minor finding, R1, in the other session's design document 15 (sampling rates described as hard limits); no JSON
  finding. Dismissed as outside this run's scope; round 2 (`2026-11-21-code-02.md`) confirmed the ownership but kept R1
  open because the range includes those commits. Not dismissed again: R1 goes to the user as a decision; the loop stops
  with no JSON finding open.
- **L14** (2026-11-21): AC-JS11 (9) done. `json_navigate`: 2,447,585 runs in 3,689 s, no failure; libFuzzer kept 6
  slow units (15–46 KB). Profiled in release without sanitizers on the largest (an array of 9,211 numbers): normalize
  1.1 ms, walking every value 8 ms, every typed read 19 ms, but `element(i)` for every element 2.2 s and a pointer from
  the root for every value 1.6 s: the target's own all-pairs checks over navigation that scans the text by design
  (the navigation index is deferred to module 17, JS6 and document 13). Not a defect. Measurements launched:
  `work/tj/all.sh` (base `cf73c36`, head `ba15c45` plus the other session's design commits, which change no code).
- **L15** (2026-11-21): measurements. First Windows `json_measure` attempt failed to launch (`cmd start /affinity`
  wrapper of `tj.py`); rerun through the bench launcher (`work/tj/win3.sh`). One-round pass of W1–W32 and 10 rounds
  of W23–W29 on both systems, then 10 rounds of every load outside ±3 % (`all2.sh`, `win3.sh`). Windows: all within
  ±3 % except W9, W12, W13 and W29, all faster on the branch. Linux: table loads spread both ways with 4–5 of 10
  rounds slower; W16 (open) 0.856 with rounds overlapping (2.1–4.6 against 2.7–5.3 ms/op). No regression repeats on
  both systems. JSON: inserting costs 1.08–2.11 times the `string` floor, reading whole 1.02–1.05 (1.46 at 64 KB on
  Linux), the last root property up to 2.85 (Windows) and 8.05 (Linux) at 64 KB. Details in the closing report.
- **L16** (2026-11-21): RDD last slice `d86eb6e..HEAD` (the commits after T4, with the Codex review records committed
  in `40e2d92`): assessed `high` (process boundary in a fuzz target), lenses readability and reliability, consent
  granted per `CLAUDE.md`, lineage `review-1c5b12feaf9b3379`.
  Approved with no blocker; advisories, none acted on: readability WARNING R2-cost-bound-contradiction (document 15, the same
  point as Codex R1, left to the user); SUGGESTIONs R2-html-flow-stale (document 15), R2-fuzz-readme-prd-link (the
  English README links the English PRD translation on purpose; the file exists), R2-measure-read-ignores-what and
  R3-scale-zero (`json_measure`, informative tool: strings are only read whole and scale 0 is not a valid input).
  Acknowledged: authority burned.
