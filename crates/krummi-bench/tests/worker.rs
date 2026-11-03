//! The Rust worker (task T2): every Rust engine prepares its base store and runs every load it runs at the reduced
//! scale, through the worker protocol, as the orchestrator starts it (`KRUMMI_BENCH_ROLE=worker`).
//!
//! Checks: exit code `0`; the operations of every repetition are those of the *Reducida* column (AC-K5); every engine
//! receives the same data as Krummi, load by load (AC-K7); canopydb and fjall are never asked to run W19–W22 and
//! refuse them (K6); and every simulated failure ends the worker with its code (AC-K10).

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::OnceLock;

use krummi_bench::measure::Repetition;
use krummi_bench::protocol::Line;
use krummi_bench::workload::{self, Engine, Load, Scale, Start};

const RUST_ENGINES: [Engine; 5] = [
    Engine::Krummi,
    Engine::Lmdb,
    Engine::Redb,
    Engine::Canopydb,
    Engine::Fjall,
];

fn worker(args: &[&OsStr], fail: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_krummi-bench"));
    command
        .env("KRUMMI_BENCH_ROLE", "worker")
        .env_remove("KRUMMI_BENCH_FAIL")
        .args(args);
    if let Some(fail) = fail {
        command.env("KRUMMI_BENCH_FAIL", fail);
    }
    command.output().expect("the worker starts")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Runs the worker and expects exit code `0` and an empty standard error; returns its protocol lines.
fn run_ok(args: &[&OsStr], what: &str) -> Vec<Line> {
    let out = worker(args, None);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{what}: stderr {}",
        text(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "{what}: stderr {}",
        text(&out.stderr)
    );
    text(&out.stdout)
        .lines()
        .map(|l| Line::parse(l).unwrap_or_else(|| panic!("{what}: not a protocol line: {l:?}")))
        .collect()
}

/// What the engine's worker printed for every load: the data digests and the repetitions.
#[derive(Default)]
struct Results {
    digests: BTreeMap<Load, u64>,
    repetitions: BTreeMap<Load, Vec<Repetition>>,
}

/// Prepares the base store of `engine` and runs every load it runs at the reduced scale.
fn run_engine(engine: Engine) -> Results {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base");
    let scale = Scale::Reduced.id();
    let lines = run_ok(
        &[
            "prepare".as_ref(),
            engine.id().as_ref(),
            scale.as_ref(),
            base.as_os_str(),
        ],
        &format!("{engine:?} prepare"),
    );
    assert!(lines.is_empty(), "prepare prints nothing: {lines:?}");

    let mut results = Results::default();
    for load in Load::ALL {
        if !load.has_process() || !workload::runs(engine, load) {
            continue;
        }
        let store = dir.path().join(load.id());
        // The reduced run has one repetition, index 0.
        let source = match load.start() {
            Start::Empty => None,
            Start::Base => Some(base.clone()),
            Start::After(previous) => Some(dir.path().join(previous.id()).join("rep-0")),
        };
        let id = load.id();
        let mut args: Vec<&OsStr> = vec![
            "run".as_ref(),
            engine.id().as_ref(),
            id.as_ref(),
            scale.as_ref(),
            store.as_os_str(),
        ];
        if let Some(source) = &source {
            args.push(source.as_os_str());
        }
        for line in run_ok(&args, &format!("{engine:?} {load:?}")) {
            match line {
                Line::Data { load, digest } => {
                    assert!(
                        results.digests.insert(load, digest).is_none(),
                        "{engine:?} {load:?}: two data lines"
                    );
                }
                Line::Rep { load, repetition } => results
                    .repetitions
                    .entry(load)
                    .or_default()
                    .push(repetition),
                Line::Version { .. } => panic!("{engine:?} {load:?}: a versions line"),
            }
        }
        // The store of the last repetition stays, closed; the loads that start from it need it.
        assert!(store.join("rep-0").is_dir(), "{engine:?} {load:?}");
        if !matches!(load, Load::W17 | Load::W19 | Load::W21) {
            remove(&store);
        }
        if let Start::After(previous) = load.start() {
            remove(&dir.path().join(previous.id()));
        }
    }
    results
}

fn remove(path: &Path) {
    std::fs::remove_dir_all(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn krummi() -> &'static Results {
    static KRUMMI: OnceLock<Results> = OnceLock::new();
    KRUMMI.get_or_init(|| run_engine(Engine::Krummi))
}

/// The loads `engine` prints lines for: those it runs, W13 in W12's process.
fn printed_loads(engine: Engine) -> Vec<Load> {
    Load::ALL
        .into_iter()
        .filter(|&l| workload::runs(engine, l))
        .collect()
}

fn check_engine(engine: Engine) {
    let reference = krummi();
    let results = if engine == Engine::Krummi {
        reference
    } else {
        &run_engine(engine)
    };
    let loads = printed_loads(engine);
    assert_eq!(
        results.digests.keys().copied().collect::<Vec<_>>(),
        loads,
        "{engine:?}: one data line per load"
    );
    assert_eq!(
        results.repetitions.keys().copied().collect::<Vec<_>>(),
        loads,
        "{engine:?}: repetitions of every load"
    );
    for load in loads {
        // AC-K7: the same bytes, in the same order, as Krummi.
        assert_eq!(
            results.digests[&load], reference.digests[&load],
            "{engine:?} {load:?}: data digest"
        );
        // K11: one measured repetition, without warm-up.
        let reps = &results.repetitions[&load];
        assert_eq!(reps.len(), 1, "{engine:?} {load:?}");
        assert_eq!(reps[0].index, 0, "{engine:?} {load:?}");
        assert!(reps[0].nanos > 0, "{engine:?} {load:?}");
        // AC-K5: the quantities of the reduced column.
        match workload::expected_operations(load, Scale::Reduced) {
            Some(expected) => {
                assert_eq!(reps[0].operations, expected, "{engine:?} {load:?}")
            }
            // W11 reads the same entries in every engine (fixed seed); W12 counts what its readers finished.
            None if load == Load::W11 => assert_eq!(
                reps[0].operations, reference.repetitions[&load][0].operations,
                "{engine:?} {load:?}"
            ),
            None => {}
        }
    }
}

#[test]
fn krummi_runs_every_load() {
    check_engine(Engine::Krummi);
    // AC-K7 across languages: the digests the Voron program printed for the same loads at the reduced scale.
    let digests = &krummi().digests;
    assert_eq!(digests[&Load::W1], 0x815d_0f5b_abf1_acd0);
    assert_eq!(digests[&Load::W22], 0xec13_19b2_5c52_8055);
}

#[test]
fn lmdb_runs_every_load_with_the_data_of_krummi() {
    check_engine(Engine::Lmdb);
}

#[test]
fn redb_runs_every_load_with_the_data_of_krummi() {
    check_engine(Engine::Redb);
}

#[test]
fn canopydb_runs_the_common_loads_with_the_data_of_krummi() {
    check_engine(Engine::Canopydb);
}

#[test]
fn fjall_runs_the_common_loads_with_the_data_of_krummi() {
    check_engine(Engine::Fjall);
}

/// Runs `args` expecting failure: the exit code and the lines of standard error.
fn fails(args: &[&OsStr], fail: Option<&str>) -> (Option<i32>, Vec<String>) {
    let out = worker(args, fail);
    let stderr = text(&out.stderr).lines().map(str::to_string).collect();
    (out.status.code(), stderr)
}

// K6: canopydb and fjall have no multi-value trees or integer keys; the benchmark never asks, and they refuse.
#[test]
fn canopydb_and_fjall_refuse_the_native_loads() {
    let dir = tempfile::tempdir().unwrap();
    for engine in [Engine::Canopydb, Engine::Fjall] {
        for load in [Load::W19, Load::W20, Load::W21, Load::W22] {
            assert!(!workload::runs(engine, load));
            let store = dir.path().join(format!("{}-{}", engine.id(), load.id()));
            let id = load.id();
            let (code, stderr) = fails(
                &[
                    "run".as_ref(),
                    engine.id().as_ref(),
                    id.as_ref(),
                    "reduced".as_ref(),
                    store.as_os_str(),
                    dir.path().as_os_str(),
                ],
                None,
            );
            assert_eq!(code, Some(1), "{engine:?} {load:?}");
            assert_eq!(stderr.len(), 1, "{engine:?} {load:?}: {stderr:?}");
            assert!(!store.exists(), "{engine:?} {load:?}");
        }
    }
}

// AC-K10: a simulated engine error, missing key or wrong sum ends the worker with code 1 and one line on standard
// error; `exit` ends it with code 3 and no message. Only the named engine and load fail.
#[test]
fn simulated_failures_end_the_worker() {
    let dir = tempfile::tempdir().unwrap();
    for engine in RUST_ENGINES {
        for kind in ["error", "missing", "checksum", "exit"] {
            let store = dir.path().join(format!("{}-{kind}", engine.id()));
            let fail = format!("{}:W1:{kind}", engine.id());
            let (code, stderr) = fails(
                &[
                    "run".as_ref(),
                    engine.id().as_ref(),
                    "W1".as_ref(),
                    "reduced".as_ref(),
                    store.as_os_str(),
                ],
                Some(&fail),
            );
            if kind == "exit" {
                assert_eq!(code, Some(3), "{engine:?} {kind}");
                assert!(stderr.is_empty(), "{engine:?} {kind}: {stderr:?}");
            } else {
                assert_eq!(code, Some(1), "{engine:?} {kind}");
                assert_eq!(stderr.len(), 1, "{engine:?} {kind}: {stderr:?}");
            }
        }
    }
    // Another load of the same engine is not affected.
    let store = dir.path().join("other");
    let out = worker(
        &[
            "run".as_ref(),
            "krummi".as_ref(),
            "W2".as_ref(),
            "reduced".as_ref(),
            store.as_os_str(),
        ],
        Some("krummi:W1:error"),
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
}

#[test]
fn versions_prints_the_lmdb_version() {
    let lines = run_ok(&["versions".as_ref()], "versions");
    let [Line::Version { component, version }] = lines.as_slice() else {
        panic!("{lines:?}");
    };
    assert_eq!(component, "lmdb");
    let parts: Vec<_> = version.split('.').collect();
    assert_eq!(parts.len(), 3, "{version}");
    assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()), "{version}");
}

#[test]
fn bad_arguments_fail_with_one_line() {
    for args in [
        &["bogus"][..],
        &["run", "voron", "W1", "reduced", "x"],
        &["run", "krummi", "W13", "reduced", "x"],
        &["run", "krummi", "W7", "reduced", "x"],
        &["run", "krummi", "W1", "big", "x"],
        &["prepare", "krummi", "reduced"],
    ] {
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        let (code, stderr) = fails(&args, None);
        assert_eq!(code, Some(1), "{args:?}");
        assert_eq!(stderr.len(), 1, "{args:?}: {stderr:?}");
    }
}
