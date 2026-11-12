//! The `krummi-bench` command (K10). The same executable is also the Rust worker: the orchestrator starts it with
//! `KRUMMI_BENCH_ROLE=worker`, which is not an option of the command (design D1); and, on Windows, the launcher
//! that limits an engine process to the performance cores, with `KRUMMI_BENCH_ROLE=limit`.

use krummi_bench::{orchestrator, worker};

fn main() {
    let args = std::env::args_os().skip(1).collect();
    let role = std::env::var_os(worker::ROLE_VARIABLE);
    let code = match role.as_ref().and_then(|r| r.to_str()) {
        Some("worker") => worker::main(args),
        Some(r) if r == orchestrator::LAUNCHER_ROLE => orchestrator::launcher_main(args),
        _ => orchestrator::main(args),
    };
    std::process::exit(code);
}
