//! Runs of the whole command, which need the .NET 10 SDK and, but for the build failure, the RavenDB 7.2.7 source in
//! `KRUMMI_BENCH_RAVENDB`. Ignored by default; the CI job `benchmark` runs them with `--include-ignored`.
//!
//! - A reduced run without `--informe`: its summary line and code (AC-K5), its report in the current folder with the
//!   name of K10 and no work subfolder left (AC-K13), the structure of the report (AC-K1), and the progress, which
//!   shows one process per load in the order of K3 (AC-K11).
//! - A Voron build that fails (AC-K9).
//! - Simulated engine failures, for a Rust engine and for Voron (AC-K10).
//!
//! Every run is reduced and builds Voron in its own work subfolder.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use krummi_bench::workload::{Engine, Load, runs};
use tempfile::TempDir;

fn ravendb() -> PathBuf {
    std::env::var_os("KRUMMI_BENCH_RAVENDB")
        .map(PathBuf::from)
        .expect("set KRUMMI_BENCH_RAVENDB to the folder with the RavenDB 7.2.7 source code")
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// A reduced run in a fresh current folder (where the default report goes) with a fresh work folder.
struct Bench {
    cwd: TempDir,
    work: TempDir,
}

impl Bench {
    fn new() -> Bench {
        Bench {
            cwd: tempfile::tempdir().unwrap(),
            work: tempfile::tempdir().unwrap(),
        }
    }

    fn run(&self, ravendb: &Path, fail: Option<&str>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_krummi-bench"));
        command
            .arg("--ravendb")
            .arg(ravendb)
            .arg("--reducido")
            .arg("--carpeta")
            .arg(self.work.path())
            .current_dir(self.cwd.path())
            .env_remove("KRUMMI_BENCH_ROLE")
            .env_remove("KRUMMI_BENCH_FAIL");
        if let Some(fail) = fail {
            command.env("KRUMMI_BENCH_FAIL", fail);
        }
        command.output().expect("the command starts")
    }

    /// Checks a run that stopped: `message` is the last line of standard error, after the progress, the code is 1,
    /// standard output is empty, there is no report and the work folder is as it was (empty).
    fn stopped(&self, out: &Output, message: &str) {
        let stderr = text(&out.stderr);
        assert_eq!(stderr.lines().last(), Some(message), "stderr: {stderr}");
        assert_eq!(out.status.code(), Some(1), "stderr: {stderr}");
        assert!(out.stdout.is_empty(), "stdout: {}", text(&out.stdout));
        assert_eq!(names(self.cwd.path()), Vec::<String>::new(), "a report");
        assert_eq!(names(self.work.path()), Vec::<String>::new(), "work");
    }
}

/// The rows of the table under `## <heading>`, without its header and separator, split into cells.
fn table(report: &str, heading: &str) -> Vec<Vec<String>> {
    let section = report
        .split(&format!("## {heading}\n"))
        .nth(1)
        .unwrap_or_else(|| panic!("no section {heading}"));
    section
        .lines()
        .skip_while(|l| !l.starts_with('|'))
        .take_while(|l| l.starts_with('|'))
        .skip(2)
        .map(|l| {
            l.trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_string())
                .collect()
        })
        .collect()
}

/// Checks a table of engines: one row per load of `loads`, in order, with `no aplica` exactly where the engine does
/// not run the load (K6) and a whole number elsewhere.
fn check_engine_cells(rows: &[Vec<String>], loads: &[Load]) {
    let shown: Vec<&str> = rows.iter().map(|r| r[0].as_str()).collect();
    let expected: Vec<&str> = loads.iter().map(|l| l.name()).collect();
    assert_eq!(shown, expected);
    for (row, &load) in rows.iter().zip(loads) {
        for (i, engine) in Engine::ALL.into_iter().enumerate() {
            let cell = &row[1 + i];
            if runs(engine, load) {
                assert!(cell.parse::<u64>().is_ok(), "{load:?} {engine:?}: {cell}");
            } else {
                assert_eq!(cell, "no aplica", "{load:?} {engine:?}");
            }
        }
    }
}

#[test]
#[ignore = "needs .NET 10 and KRUMMI_BENCH_RAVENDB"]
fn a_reduced_run_writes_its_report_and_leaves_no_store() {
    let ravendb = ravendb();
    let bench = Bench::new();
    let out = bench.run(&ravendb, None);
    let stderr = text(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "stderr: {stderr}");

    // AC-K5 and AC-K13: the summary names the report, which is in the current folder with the name of K10.
    let stdout = text(&out.stdout);
    let name = stdout
        .strip_prefix("Ejecución reducida, sin umbrales. Informe: ")
        .and_then(|s| s.strip_suffix('\n'))
        .map(|s| s.trim_end_matches('\r'))
        .unwrap_or_else(|| panic!("stdout: {stdout:?}"));
    let stamp = name
        .strip_prefix("banco-")
        .and_then(|s| s.strip_suffix(".md"))
        .unwrap_or_else(|| panic!("name: {name}"));
    assert!(
        stamp.len() == 15
            && stamp.as_bytes()[8] == b'-'
            && stamp.chars().filter(char::is_ascii_digit).count() == 14,
        "name: {name}"
    );
    assert_eq!(names(bench.cwd.path()), [name]);
    assert_eq!(names(bench.work.path()), Vec::<String>::new(), "work");

    // AC-K1's structure: the 22 loads and the 8 sizes, `no aplica` only for canopydb and fjall in W19–W22.
    let report = fs::read_to_string(bench.cwd.path().join(name)).unwrap();
    assert!(report.starts_with("# Banco comparativo de Krummi\n"));
    assert!(report.contains("\n- Ejecución: reducida\n"));
    assert!(report.contains("\n- Repeticiones: 1 medida\n"));
    assert!(report.contains("\n## Resultado\n\nEjecución reducida, sin umbrales.\n"));
    let engines = report
        .lines()
        .find(|l| l.starts_with("- Motores: "))
        .expect("the engines line");
    assert!(
        engines.starts_with("- Motores: Voron 7.2.7 (.NET 10."),
        "{engines}"
    );
    assert!(!engines.contains("desconocido"), "{engines}");
    let rates = table(&report, "Operaciones por segundo");
    check_engine_cells(&rates, &Load::ALL);
    for row in &rates {
        assert_eq!(row.len(), 9, "{row:?}");
        // Two decimals with a comma, or `∞` when Voron's metric is 0 and Krummi's is not (W12 when no Voron read
        // ends while its writer writes, R4 of the Codex code review).
        let ratio = row[7].split_once(',');
        let decimal = ratio.is_some_and(|(units, hundredths)| {
            !units.is_empty()
                && units.bytes().all(|b| b.is_ascii_digit())
                && hundredths.len() == 2
                && hundredths.bytes().all(|b| b.is_ascii_digit())
        });
        assert!(decimal || row[7] == "∞", "ratio {row:?}");
        assert_eq!(row[8], "sin evaluar", "{row:?}");
    }
    let sizes = table(&report, "Tamaño del almacén (bytes)");
    check_engine_cells(&sizes, &Load::SIZED);
    assert_eq!(report.matches("no aplica").count(), 4 * 2 + 2 * 2);

    // AC-K11: one progress line per process, engines in the order of K3 and loads in table order.
    let processes: Vec<(String, String)> = stderr
        .lines()
        .filter(|l| l.contains(" op/s"))
        .map(|l| {
            let mut words = l.split_whitespace();
            let engine = words.next().unwrap().to_string();
            let load = words.next().unwrap().trim_end_matches(':').to_string();
            (engine, load)
        })
        .collect();
    let expected: Vec<(String, String)> = Engine::ALL
        .into_iter()
        .flat_map(|e| {
            Load::ALL
                .into_iter()
                .filter(move |&l| l.has_process() && runs(e, l))
                .map(move |l| (e.name().to_string(), l.name().to_string()))
        })
        .collect();
    assert_eq!(processes, expected, "stderr: {stderr}");
}

#[test]
#[ignore = "needs .NET 10"]
fn a_failed_voron_build_stops_the_run() {
    // A RavenDB folder that passes the checks, whose Voron project cannot be loaded.
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    fs::create_dir_all(root.join("src/Voron")).unwrap();
    fs::write(
        root.join("src/Voron/Voron.csproj"),
        "esto no es un proyecto",
    )
    .unwrap();
    fs::write(
        root.join("src/CommonAssemblyInfo.cs"),
        "[assembly: AssemblyInformationalVersion(\"7.2.7\")]",
    )
    .unwrap();
    fs::write(
        root.join("NuGet.Config"),
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<configuration><packageSources><clear />\
         <add key=\"nuget.org\" value=\"https://api.nuget.org/v3/index.json\" /></packageSources></configuration>\n",
    )
    .unwrap();
    let bench = Bench::new();
    let out = bench.run(root, None);
    let stderr = text(&out.stderr);
    let last = stderr.lines().last().unwrap_or_default().to_string();
    assert!(
        last.starts_with("No se pudo compilar Voron: ")
            && last.contains("error")
            && last.ends_with('.')
            && !last.ends_with(".."),
        "stderr: {stderr}"
    );
    bench.stopped(&out, &last);
}

/// The cause of the simulated failure `missing`: the key of index `u64::MAX` is not found.
fn missing_cause() -> String {
    let hex: String = krummi_bench::data::key(u64::MAX)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("no se encuentra la clave {hex}")
}

/// A run with the simulated failure `fail` stops with `El motor <engine> falló en la carga <load>: <cause>.`
fn fails(fail: &str, engine: Engine, load: Load, cause: &str) {
    let ravendb = ravendb();
    let bench = Bench::new();
    let out = bench.run(&ravendb, Some(fail));
    let message = format!(
        "El motor {} falló en la carga {}: {cause}.",
        engine.name(),
        load.name()
    );
    bench.stopped(&out, &message);
}

const CHECKSUM_CAUSE: &str = "la suma de comprobación es 1 y se esperaba 0";
const EXIT_CAUSE: &str = "el proceso terminó con el código 3 sin terminar la carga";

#[test]
#[ignore = "needs .NET 10 and KRUMMI_BENCH_RAVENDB"]
fn a_rust_engine_that_misses_a_key_stops_the_run() {
    fails(
        "krummi:W7:missing",
        Engine::Krummi,
        Load::W7,
        &missing_cause(),
    );
}

#[test]
#[ignore = "needs .NET 10 and KRUMMI_BENCH_RAVENDB"]
fn a_rust_engine_with_a_wrong_sum_stops_the_run_named_by_its_process() {
    // W13 runs in W12's process, which names the failure.
    fails(
        "krummi:W13:checksum",
        Engine::Krummi,
        Load::W12,
        CHECKSUM_CAUSE,
    );
}

#[test]
#[ignore = "needs .NET 10 and KRUMMI_BENCH_RAVENDB"]
fn a_rust_engine_that_ends_without_a_message_stops_the_run() {
    fails("krummi:W4:exit", Engine::Krummi, Load::W4, EXIT_CAUSE);
}

#[test]
#[ignore = "needs .NET 10 and KRUMMI_BENCH_RAVENDB"]
fn voron_missing_a_key_stops_the_run() {
    fails(
        "voron:W7:missing",
        Engine::Voron,
        Load::W7,
        &missing_cause(),
    );
}

#[test]
#[ignore = "needs .NET 10 and KRUMMI_BENCH_RAVENDB"]
fn voron_with_a_wrong_sum_stops_the_run() {
    fails(
        "voron:W13:checksum",
        Engine::Voron,
        Load::W12,
        CHECKSUM_CAUSE,
    );
}

#[test]
#[ignore = "needs .NET 10 and KRUMMI_BENCH_RAVENDB"]
fn voron_ending_without_a_message_stops_the_run() {
    fails("voron:W4:exit", Engine::Voron, Load::W4, EXIT_CAUSE);
}
