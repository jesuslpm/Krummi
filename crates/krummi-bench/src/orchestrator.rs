//! The orchestrator, the command of K10: the checks of *Validaciones y mensajes* in their order, the Voron build
//! (design D2), the plan of K3 one worker process at a time, and the report (*Salida*). Every exit after the work
//! subfolder exists, also a failure or Ctrl+C, removes that subfolder.

mod child;
mod machine;
mod plan;

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};

use crate::cli::{self, Failure, Options};
use crate::report::{self, Run, Versions};
use crate::workload::{Engine, Scale};
use child::Ended;
use plan::Workers;

/// The project of the Voron program (design D1).
const VORON_PROJECT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/voron/VoronBench.csproj");

/// Why a run stops before its end.
pub(crate) enum Stop {
    Failed(Failure),
    Interrupted,
}

impl From<Failure> for Stop {
    fn from(failure: Failure) -> Stop {
        Stop::Failed(failure)
    }
}

/// Runs the command with `args` and returns the exit code.
pub fn main(args: Vec<OsString>) -> i32 {
    let started = chrono::Local::now().fixed_offset();
    let checked = check(&args, &started);
    let (options, report, report_shown) = match checked {
        Ok(checked) => checked,
        Err(failure) => {
            eprintln!("{}", failure.message);
            return failure.code;
        }
    };
    child::watch_interrupt();
    let work = options.work.clone().unwrap_or_else(std::env::temp_dir);
    let name = format!(
        "krummi-bench-{}-{}",
        started.format("%Y%m%d-%H%M%S"),
        std::process::id()
    );
    let sub = match cli::create_work_subfolder(&work, &name) {
        Ok(sub) => sub,
        Err(failure) => {
            eprintln!("{}", failure.message);
            return failure.code;
        }
    };
    let result = execute(&options, &work, &sub, started, &report, &report_shown);
    remove_subfolder(&sub);
    match result {
        Ok((summary, code)) => {
            println!("{summary}");
            code
        }
        Err(Stop::Failed(failure)) => {
            eprintln!("{}", failure.message);
            failure.code
        }
        Err(Stop::Interrupted) => {
            eprintln!("Banco interrumpido.");
            1
        }
    }
}

/// The checks before the work folder, in the order of the table: the arguments, the report, RavenDB and .NET.
/// Returns the options and the report's file with the text the messages show.
fn check(
    args: &[OsString],
    started: &DateTime<FixedOffset>,
) -> Result<(Options, PathBuf, String), Failure> {
    let options = cli::parse(args)?;
    let (report, shown) = match &options.report {
        Some(path) => (path.clone(), cli::shown(path)),
        None => {
            let name = report::default_report_name(started);
            (PathBuf::from(&name), name)
        }
    };
    cli::check_report(&report)?;
    cli::check_ravendb(&options.ravendb)?;
    let sdks = Command::new("dotnet")
        .arg("--list-sdks")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    cli::check_sdk(sdks.as_deref())?;
    Ok((options, report, shown))
}

/// Everything after the work subfolder exists: returns the summary line and the exit code.
fn execute(
    options: &Options,
    work: &Path,
    sub: &Path,
    started: DateTime<FixedOffset>,
    report: &Path,
    report_shown: &str,
) -> Result<(String, i32), Stop> {
    let scale = if options.reduced {
        Scale::Reduced
    } else {
        Scale::Full
    };
    let workers = Workers {
        // If its own path is unknown, the Rust worker fails to start, which is reported as Krummi's failure.
        rust: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("krummi-bench")),
        voron: build_voron(&options.ravendb, sub)?,
    };
    let lmdb = workers.version(Engine::Lmdb, "lmdb")?;
    let dotnet = workers.version(Engine::Voron, "dotnet")?;
    let (krummi_commit, krummi_dirty) = machine::krummi_commit();
    let versions = Versions {
        krummi_commit,
        krummi_dirty,
        dotnet,
        lmdb,
        rust: machine::rust(),
    };
    let machine = machine::machine(work);
    if child::interrupted() {
        return Err(Stop::Interrupted);
    }

    let measured = plan::run(&workers, sub, scale)?;

    let run = Run {
        started,
        scale,
        machine,
        versions,
        metrics: measured.metrics,
        sizes: measured.sizes,
        changed_options: report::changed_options(),
    };
    write_report(report, &run.render()).map_err(|e| {
        Failure::new(
            format!(
                "No se pudo escribir el informe {report_shown}: {}.",
                one_line(&e.to_string())
            ),
            1,
        )
    })?;
    Ok((run.summary_line(report_shown), run.exit_code()))
}

/// Builds the Voron program in `<sub>/dotnet`, with `sub` as current folder, so that the repository and the
/// RavenDB folder keep their files (design D2). Returns the program's assembly.
fn build_voron(ravendb: &Path, sub: &Path) -> Result<PathBuf, Stop> {
    let absolute = |p: &Path| std::path::absolute(p).map_err(|e| build_failure(&e.to_string()));
    let ravendb = absolute(ravendb)?;
    let sub = absolute(sub)?;
    let artifacts = sub.join("dotnet");
    let root = format!("-p:RavenDbRoot={}", ravendb.display());
    let config = ravendb.join("NuGet.Config");
    let common = |verb: &str| -> Vec<OsString> {
        vec![
            verb.into(),
            VORON_PROJECT.into(),
            "--artifacts-path".into(),
            artifacts.clone().into(),
            root.clone().into(),
            "--disable-build-servers".into(),
            "--nologo".into(),
        ]
    };
    let mut restore = common("restore");
    restore.extend(["--configfile".into(), config.into()]);
    let mut build = common("build");
    build.extend(["-c".into(), "Release".into(), "--no-restore".into()]);

    eprintln!("Compilando Voron…");
    for args in [restore, build] {
        let mut command = Command::new("dotnet");
        command
            .args(&args)
            .current_dir(&sub)
            .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
            .env("DOTNET_NOLOGO", "1");
        match child::run(&mut command) {
            Ended::Interrupted => return Err(Stop::Interrupted),
            Ended::NotStarted(e) => return Err(build_failure(&e.to_string())),
            Ended::Finished(f) if f.code == Some(0) => {}
            Ended::Finished(f) => {
                let error = f
                    .stdout
                    .lines()
                    .chain(f.stderr.lines())
                    .find(|l| l.contains(" error "))
                    .map(str::to_string);
                let code = f.code.map_or("desconocido".to_string(), |c| c.to_string());
                let cause = error.unwrap_or_else(|| format!("dotnet terminó con el código {code}"));
                return Err(build_failure(&cause));
            }
        }
    }
    let assembly = artifacts.join("bin/VoronBench/release/VoronBench.dll");
    if !assembly.is_file() {
        return Err(build_failure(&format!(
            "no se encuentra {}",
            assembly.display()
        )));
    }
    Ok(assembly)
}

/// `No se pudo compilar Voron: <causa>.`
fn build_failure(cause: &str) -> Stop {
    Stop::Failed(Failure::new(
        format!("No se pudo compilar Voron: {}.", one_line(cause)),
        1,
    ))
}

/// A cause as the messages show it: one line, without the final period the message adds.
pub(crate) fn one_line(text: &str) -> String {
    let joined: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    joined
        .join(" ")
        .trim_end_matches('.')
        .trim_end()
        .to_string()
}

/// Writes the report to a new file, never over an existing one; on an error, removes what it wrote.
fn write_report(path: &Path, text: &str) -> io::Result<()> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let written = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all());
    if written.is_err() {
        drop(file);
        let _ = fs::remove_file(path);
    }
    written
}

/// Removes the work subfolder. A child that was just killed may hold its files for a moment on Windows, so it
/// tries a few times.
fn remove_subfolder(sub: &Path) {
    for attempt in 0..20 {
        match fs::remove_dir_all(sub) {
            Ok(()) => return,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return,
            Err(_) if attempt < 19 => thread::sleep(Duration::from_millis(250)),
            Err(e) => eprintln!("No se pudo borrar la subcarpeta {}: {e}", sub.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cause_is_one_line_without_its_final_period() {
        assert_eq!(one_line("  la clave no está.\r\n"), "la clave no está");
        assert_eq!(one_line("uno\n\n dos.\n"), "uno dos");
        assert_eq!(one_line(""), "");
    }
}
