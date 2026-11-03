//! The `krummi-bench` command (K10). The same executable is also the Rust worker: the orchestrator starts it with
//! `KRUMMI_BENCH_ROLE=worker`, which is not an option of the command (design D1).

fn main() {
    let code = if std::env::var_os(krummi_bench::worker::ROLE_VARIABLE).is_some_and(|v| v == "worker") {
        krummi_bench::worker::main(std::env::args_os().skip(1).collect())
    } else {
        krummi_bench::orchestrator::main(std::env::args_os().skip(1).collect())
    };
    std::process::exit(code);
}
