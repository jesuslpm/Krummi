//! The measurement plan (K3, AC-K11): every engine in turn, and within each, its base store and then one worker
//! process per load in table order (W12 and W13 in one). The workers speak the protocol of `crate::protocol`.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::Stop;
use super::child::{self, Ended};
use crate::cli::Failure;
use crate::measure::{Repetition, load_metric};
use crate::protocol::Line;
use crate::size::folder_size;
use crate::worker::ROLE_VARIABLE;
use crate::workload::{Engine, Load, Scale, Start, expected_operations, runs};

/// How the orchestrator starts each worker.
pub struct Workers {
    /// The Rust worker: this executable.
    pub rust: PathBuf,
    /// The Voron program, run with `dotnet`.
    pub voron: PathBuf,
}

impl Workers {
    pub fn command(&self, engine: Engine) -> Command {
        if engine == Engine::Voron {
            let mut command = Command::new("dotnet");
            command.arg(&self.voron);
            command
        } else {
            let mut command = Command::new(&self.rust);
            command.env(ROLE_VARIABLE, "worker");
            command
        }
    }

    /// The version a worker's `versions` command prints for `component`; `None` if it cannot say.
    pub fn version(&self, engine: Engine, component: &str) -> Result<Option<String>, Stop> {
        match child::run(self.command(engine).arg("versions")) {
            Ended::Interrupted => Err(Stop::Interrupted),
            Ended::Finished(f) if f.code == Some(0) => {
                Ok(f.stdout.lines().find_map(|l| match Line::parse(l) {
                    Some(Line::Version {
                        component: c,
                        version,
                    }) if c == component => Some(version),
                    _ => None,
                }))
            }
            _ => Ok(None),
        }
    }
}

/// What the plan measured: the metric of every load and engine that ran it, and the sizes of K9.
#[derive(Default)]
pub struct Measured {
    pub metrics: BTreeMap<(Load, Engine), f64>,
    pub sizes: BTreeMap<(Load, Engine), u64>,
}

/// Runs the whole plan with its stores under `sub`, strictly one process at a time.
pub fn run(workers: &Workers, sub: &Path, scale: Scale) -> Result<Measured, Stop> {
    let mut measured = Measured::default();
    // Krummi runs first and every load: its data digests are the reference of every other engine (AC-K7).
    let mut reference = BTreeMap::new();
    for engine in Engine::ALL {
        run_engine(workers, sub, scale, engine, &mut reference, &mut measured)?;
    }
    Ok(measured)
}

fn run_engine(
    workers: &Workers,
    sub: &Path,
    scale: Scale,
    engine: Engine,
    reference: &mut BTreeMap<Load, u64>,
    measured: &mut Measured,
) -> Result<(), Stop> {
    let (warmups, repetitions) = scale.repetitions();
    let last = warmups + repetitions - 1;
    let last_store = |store: &Path| store.join(format!("rep-{last}"));
    let loads: Vec<Load> = Load::ALL
        .into_iter()
        .filter(|&l| l.has_process() && runs(engine, l))
        .collect();
    let dir = sub.join(engine.id());
    let base = dir.join("base");

    // A failure preparing the base store is one of the first load that uses it.
    let first_on_base = loads
        .iter()
        .copied()
        .find(|l| l.start() == Start::Base)
        .expect("every engine runs loads on the base store");
    worker(
        workers,
        engine,
        first_on_base,
        &[
            "prepare".into(),
            engine.id().into(),
            scale.id().into(),
            base.clone().into(),
        ],
    )?;
    eprintln!("{}: almacén base preparado", engine.name());

    for &load in &loads {
        let store = dir.join(load.id());
        let mut args: Vec<OsString> = vec![
            "run".into(),
            engine.id().into(),
            load.id().into(),
            scale.id().into(),
            store.clone().into(),
        ];
        match load.start() {
            Start::Empty => {}
            Start::Base => args.push(base.clone().into()),
            Start::After(previous) => args.push(last_store(&dir.join(previous.id())).into()),
        }
        let stdout = worker(workers, engine, load, &args)?;
        let fail = |cause: &str| Stop::Failed(engine_failure(engine, load, cause));
        let results = read_output(&stdout, load, scale).map_err(|c| fail(&c))?;

        let mut progress = Vec::new();
        for (printed, (reps, digest)) in results {
            match reference.get(&printed) {
                None if engine == Engine::Krummi => {
                    reference.insert(printed, digest);
                }
                Some(&expected) if expected == digest => {}
                _ => return Err(fail("los datos no coinciden con los de Krummi")),
            }
            let metric = load_metric(&reps, warmups);
            measured.metrics.insert((printed, engine), metric);
            progress.push(format!("{}: {metric:.0} op/s", printed.name()));
        }
        if Load::SIZED.contains(&load) {
            let size = folder_size(&last_store(&store)).map_err(|e| fail(&e.to_string()))?;
            measured.sizes.insert((load, engine), size);
        }
        // A store that another load starts from (W17, W19 and W21) stays until that load has run.
        if !Load::ALL.iter().any(|l| l.start() == Start::After(load)) {
            remove(&store).map_err(|c| fail(&c))?;
        }
        if let Start::After(previous) = load.start() {
            remove(&dir.join(previous.id())).map_err(|c| fail(&c))?;
        }
        eprintln!("{} {}", engine.name(), progress.join("; "));
    }
    let last_load = *loads.last().expect("every engine runs loads");
    remove(&dir).map_err(|c| Stop::Failed(engine_failure(engine, last_load, &c)))
}

/// Runs one worker process, attributed to `load`, and returns its standard output.
fn worker(
    workers: &Workers,
    engine: Engine,
    load: Load,
    args: &[OsString],
) -> Result<String, Stop> {
    let fail = |cause: &str| Stop::Failed(engine_failure(engine, load, cause));
    match child::run(workers.command(engine).args(args)) {
        Ended::Interrupted => Err(Stop::Interrupted),
        Ended::NotStarted(e) => Err(fail(&e.to_string())),
        Ended::Finished(f) if f.code == Some(0) => Ok(f.stdout),
        Ended::Finished(f) => {
            let cause = super::one_line(&f.stderr);
            if cause.is_empty() {
                let code = f.code.map_or("desconocido".to_string(), |c| c.to_string());
                Err(fail(&format!(
                    "el proceso terminó con el código {code} sin terminar la carga"
                )))
            } else {
                Err(fail(&cause))
            }
        }
    }
}

/// `El motor <motor> falló en la carga <carga>: <causa>.`
fn engine_failure(engine: Engine, load: Load, cause: &str) -> Failure {
    Failure::new(
        format!(
            "El motor {} falló en la carga {}: {}.",
            engine.name(),
            load.name(),
            super::one_line(cause)
        ),
        1,
    )
}

/// The repetitions and the data digest of every load a process printed (W12's prints W12 and W13), checked: one
/// data line per load, repetitions numbered from 0 as the scale says, and the operations of the table (AC-K5).
fn read_output(
    stdout: &str,
    load: Load,
    scale: Scale,
) -> Result<BTreeMap<Load, (Vec<Repetition>, u64)>, String> {
    let printed: &[Load] = if load == Load::W12 {
        &[Load::W12, Load::W13]
    } else {
        &[load]
    };
    let mut repetitions: BTreeMap<Load, Vec<Repetition>> = BTreeMap::new();
    let mut digests: BTreeMap<Load, u64> = BTreeMap::new();
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let invalid = || format!("el proceso escribió una línea no válida: {}", line.trim());
        match Line::parse(line).ok_or_else(invalid)? {
            Line::Data { load, digest } if printed.contains(&load) => {
                if digests.insert(load, digest).is_some() {
                    return Err(invalid());
                }
            }
            Line::Rep { load, repetition } if printed.contains(&load) => {
                repetitions.entry(load).or_default().push(repetition);
            }
            _ => return Err(invalid()),
        }
    }
    let (warmups, measured) = scale.repetitions();
    let mut results = BTreeMap::new();
    for &l in printed {
        let reps = repetitions.remove(&l).unwrap_or_default();
        if !reps.iter().map(|r| r.index).eq(0..warmups + measured) {
            return Err(format!(
                "el proceso no dio las repeticiones esperadas de la carga {}",
                l.name()
            ));
        }
        if let Some(expected) = expected_operations(l, scale)
            && let Some(r) = reps.iter().find(|r| r.operations != expected)
        {
            return Err(format!(
                "el número de operaciones es {} y se esperaba {expected}",
                r.operations
            ));
        }
        let digest = digests.remove(&l).ok_or_else(|| {
            format!(
                "el proceso no dio la suma de los datos de la carga {}",
                l.name()
            )
        })?;
        results.insert(l, (reps, digest));
    }
    Ok(results)
}

/// Removes a store folder and everything in it.
fn remove(path: &Path) -> Result<(), String> {
    fs::remove_dir_all(path).map_err(|e| format!("no se puede borrar {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_output_of_a_process_is_checked() {
        let ok = "data W12 00ff\nrep 0 W12 7 10\nrep 0 W13 1000 10\ndata W13 01\n";
        let results = read_output(ok, Load::W12, Scale::Reduced).unwrap();
        assert_eq!(results[&Load::W12].1, 0xff);
        assert_eq!(results[&Load::W13].0[0].operations, 1_000);

        let fails = |out: &str| read_output(out, Load::W4, Scale::Reduced).unwrap_err();
        assert_eq!(
            fails("data W4 1\nrep 0 W4 159 10\n"),
            "el número de operaciones es 159 y se esperaba 160"
        );
        assert_eq!(
            fails("data W4 1\nrep 1 W4 160 10\n"),
            "el proceso no dio las repeticiones esperadas de la carga escrituras-sueltas"
        );
        assert_eq!(
            fails("rep 0 W4 160 10\n"),
            "el proceso no dio la suma de los datos de la carga escrituras-sueltas"
        );
        assert_eq!(
            fails("data W5 1\n"),
            "el proceso escribió una línea no válida: data W5 1"
        );
        assert_eq!(
            fails("hola\n"),
            "el proceso escribió una línea no válida: hola"
        );
        // Full scale: the warm-up and three measured repetitions.
        let full =
            "data W4 1\nrep 0 W4 10000 1\nrep 1 W4 10000 1\nrep 2 W4 10000 1\nrep 3 W4 10000 1\n";
        assert!(read_output(full, Load::W4, Scale::Full).is_ok());
    }
}
