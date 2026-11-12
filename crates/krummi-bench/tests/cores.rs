//! The launcher that limits an engine process to the performance cores on Windows (B4 of the Krummi and Voron only
//! PRD; R1 of its Codex code review): the benchmark's executable with `KRUMMI_BENCH_ROLE=limit` joins a job limited
//! to `KRUMMI_BENCH_LIMIT_MASK` and only then starts the engine process, which is created inside the job; when it
//! cannot, it says so and starts nothing.
#![cfg(windows)]

use std::process::{Command, Output};

/// The affinity of this process.
fn own_mask() -> u64 {
    let current = win32job::utils::get_current_process();
    win32job::utils::get_process_affinity_mask(current)
        .unwrap()
        .0 as u64
}

/// The launcher with `mask`, running a PowerShell that prints its own affinity and then that of a PowerShell it starts.
fn launch(mask: &str) -> Output {
    let own = "[Diagnostics.Process]::GetCurrentProcess().ProcessorAffinity.ToInt64()";
    Command::new(env!("CARGO_BIN_EXE_krummi-bench"))
        .env("KRUMMI_BENCH_ROLE", "limit")
        .env("KRUMMI_BENCH_LIMIT_MASK", mask)
        .env("KRUMMI_BENCH_LIMIT_ROLE", "worker")
        .args([
            "powershell",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                "$env:KRUMMI_BENCH_ROLE; {own}; powershell -NoProfile -NonInteractive -Command '{own}'"
            ),
        ])
        .output()
        .expect("the launcher starts")
}

fn lines(out: &Output) -> Vec<String> {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim_end().to_string())
        .collect()
}

// AC-B3: the engine process and the process it starts run on the mask's cores from their creation, the engine
// process gets the role the launcher was given for it, and the benchmark's own affinity does not change.
#[test]
fn the_launcher_starts_the_process_inside_the_limit() {
    let before = own_mask();
    let core = before.trailing_zeros();
    let mask = 1u64 << core;
    let out = launch(&mask.to_string());
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        lines(&out),
        ["limit 1", "worker", &mask.to_string(), &mask.to_string()]
    );
    assert_eq!(own_mask(), before);
}

// B4: a launcher that cannot set the limit says so and runs nothing: the benchmark then starts the process itself.
#[test]
fn a_launcher_without_its_limit_runs_nothing() {
    let out = launch("not a mask");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(lines(&out), ["limit 0"]);
}

// The launcher ends with the engine process's exit code.
#[test]
fn the_launcher_ends_with_the_process_code() {
    let out = Command::new(env!("CARGO_BIN_EXE_krummi-bench"))
        .env("KRUMMI_BENCH_ROLE", "limit")
        .env("KRUMMI_BENCH_LIMIT_MASK", own_mask().to_string())
        .args([
            "powershell",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "exit 7",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(lines(&out), ["limit 1"]);
}
