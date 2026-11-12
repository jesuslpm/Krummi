# Krummi and Voron only benchmark code review resolutions

Resolutions of [2026-11-12-code.md](2026-11-12-code.md), the review of `cac715b..1e9dffc` (the ODD run of
`docs/prd/krummi/benchmark-krummi-voron.prd.md`). Append-only: each update is dated and keeps the earlier ones. A
claimed fix stays unverified until a follow-up review confirms it.

## 2026-11-12

Artifact versions: fixes in commit `a65a111`, on branch `feat/table-row-reads`.

### R1 — blocker: Windows workers can execute before their affinity limit is applied

- Disposition: fixed.
- Evidence: the finding holds: `spawn_limited` started the process and only then assigned it to the job, with no
  ordering between the two; D2 of the execution document assumed the window did not matter, which B4 does not allow.
- Fix (`a65a111`): on Windows the engine process starts through the benchmark's own executable as a launcher
  (`KRUMMI_BENCH_ROLE=limit`, `orchestrator::cores::launcher_main`). The launcher joins a job with the affinity limit
  and *kill on job close*, writes `limit 1` (or `limit 0` when it cannot) as its first line of output, and only then
  starts the engine process, which is created inside the job, as are the processes it starts. `Pin::finish` takes
  that line out of the output and records a process without the limit; the job's handle stays open until the launcher
  exits (closing it earlier ended the launcher, which is in the job, with code 0; found by a test). Stopping the
  launcher (Ctrl+C) closes the job and ends the engine process. Linux keeps its spawn from a limited thread, whose
  affinity the process inherits at creation.
- Tests: `tests/cores.rs` runs the binary as launcher: `the_launcher_starts_the_process_inside_the_limit` (the
  process and the one it starts print the job's mask, the role reaches the engine process, the test's affinity does
  not change), `a_launcher_without_its_limit_runs_the_process_unlimited` (`limit 0`, full affinity) and
  `the_launcher_ends_with_the_process_code` (code 7). Unit tests `the_launcher_runs_the_same_command_with_its_role_and_mask`
  and `the_launcher_line_is_taken_out_of_the_output`. The ignored runs `a_krummi_and_voron_only_run_measures_only_those_two`,
  `a_krummi_and_voron_only_run_stops_on_a_failure` and `a_relative_work_folder_holds_every_store` pass on Windows
  through the launcher (621 s).
- Status: claimed fixed, unverified until a follow-up review.

### R2 — minor: the integration test rejects the permitted unknown-core fallback

- Disposition: fixed.
- Fix (`a65a111`): `a_krummi_and_voron_only_run_measures_only_those_two` accepts the three lines of B5, with numbers in
  the performance line.
- Status: claimed fixed, unverified until a follow-up review.

Checks after the fixes: `cargo fmt --all --check`; `cargo clippy -p krummi-bench --all-targets -- -D warnings` clean on
Windows and on WSL 2 Ubuntu; `cargo test -p krummi-bench` passes on both (59 and 58 unit tests: the launcher-line test
is Windows-only).

Review record: [2026-11-12-code.md](2026-11-12-code.md), wrapper status `findings`, thread
`01a1d142-f546-7f10-8f1f-40d0f4a219ab`; telemetry: coordinator `gpt-6.1-sol` / `high`, reviewer
`krummi-review-critical` `gpt-6-astra` / `xhigh`.

## 2026-11-12 (after the RDD review of the fixes)

Artifact versions: commit `b02b34b`. The RDD review of `93ba937..440f8ac` (lenses risk, resilience and reliability,
lineage `review-57046d6b0ba0caf5`, approved) left warnings on R1's fix, applied here:

- A launcher that cannot join its job no longer starts the engine process: it writes `limit 0` and ends. The
  benchmark reads the launcher's first line when it starts it; unless it is `limit 1`, or when the launcher cannot
  start, it starts the engine process itself without the limit and records it (`Núcleos: desconocido`). No engine
  process is left outside the benchmark's control, and B4's fallback holds when the launcher fails.
- The Windows chain is tested through the run's own code: `a_limited_process_and_the_ones_it_starts_run_on_its_cores`
  (`Pin` with the real launcher built next to the tests; a mutation that starts the process outside the launcher
  fails it with `["4294967295", "4294967295"]` against `["1", "1"]`) and
  `a_process_whose_launcher_cannot_start_runs_without_the_limit`; `a_launcher_without_its_limit_runs_nothing`.
- Kept: Ctrl+C now ends the engine process through the job instead of the console signal; the run removes its work
  subfolder on an interruption anyway (AC-K9), so no store survives it. The run test keeps accepting
  `Núcleos: desconocido` (R2), since the chain test above checks the limit itself.

Checks: `cargo fmt --all --check`; clippy clean and `cargo test -p krummi-bench` passing on Windows and WSL 2 Ubuntu; the
ignored `a_krummi_and_voron_only_run_measures_only_those_two` passes on Windows (217 s).
