//! Krummi comparative benchmark (module 6, [benchmark.prd.md](../../../docs/prd/krummi/benchmark.prd.md)).
//!
//! The library holds what the orchestrator (`krummi-bench`, the command of K10) and the Rust worker (the same
//! executable started with `KRUMMI_BENCH_ROLE=worker`) share: the loads and their quantities, the data, the worker protocol, the metrics and
//! thresholds, the report and the checks of the command. The .NET program for Voron in `voron/` mirrors `data`,
//! `workload` and `protocol`.

pub mod cli;
pub mod data;
pub mod engines;
pub mod measure;
pub mod orchestrator;
pub mod protocol;
pub mod report;
pub mod size;
pub mod worker;
pub mod workload;
