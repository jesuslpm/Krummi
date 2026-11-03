//! The Rust worker (task T2): runs one load of one Rust engine and prints the protocol lines.

use std::ffi::OsString;

/// The environment variable that makes the executable a worker (design D1).
pub const ROLE_VARIABLE: &str = "KRUMMI_BENCH_ROLE";

/// Runs the worker command in `args` and returns the exit code.
pub fn main(_args: Vec<OsString>) -> i32 {
    unimplemented!("task T2")
}
