//! The performance cores (B4 and B5 of the Krummi and Voron only PRD): which logical cores the engine processes run
//! on, and the limit that keeps them there. The classes come from the Voron program's `cores` command (one
//! `core <id> <class>` line per logical core, a higher class being a faster core). The limit uses safe APIs only
//! (K14) and holds from the process's creation: on Windows, the benchmark's executable starts first as a launcher
//! (`KRUMMI_BENCH_ROLE=limit`, [`launcher_main`]) that joins a job object with an affinity limit and only then
//! starts the engine process, which is created in the job, as every process it starts; on Linux, the process
//! inherits the affinity of the thread that starts it.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::report::Cores;

/// Where the engines are measured, decided before the first engine process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// The system gives more than one class: the logical cores of the fastest, out of `total`.
    Performance { cores: Vec<usize>, total: usize },
    /// The system gives one class, or none: the processes are not limited.
    Unpinned,
    /// The classes could not be asked for: the processes are not limited.
    Unknown,
}

/// The placement of the `classes` the system gives, `(logical core, class)`; `None` when they could not be asked for.
pub fn placement(classes: Option<&[(usize, u32)]>) -> Placement {
    let Some(classes) = classes else {
        return Placement::Unknown;
    };
    let distinct: BTreeSet<u32> = classes.iter().map(|&(_, class)| class).collect();
    match distinct.last() {
        Some(&fastest) if distinct.len() > 1 => {
            let mut cores: Vec<usize> = classes
                .iter()
                .filter(|&&(_, class)| class == fastest)
                .map(|&(core, _)| core)
                .collect();
            cores.sort_unstable();
            Placement::Performance {
                cores,
                total: classes.len(),
            }
        }
        _ => Placement::Unpinned,
    }
}

/// The classes in the output of the `cores` command: `None` if a line is not `core <id> <class>` or a core repeats.
pub fn parse_classes(stdout: &str) -> Option<Vec<(usize, u32)>> {
    let mut classes = Vec::new();
    let mut seen = BTreeSet::new();
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        match line.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["core", core, class] => {
                let core: usize = core.parse().ok()?;
                if !seen.insert(core) {
                    return None;
                }
                classes.push((core, class.parse().ok()?));
            }
            _ => return None,
        }
    }
    Some(classes)
}

impl Placement {
    /// The report's *Núcleos* line once every engine process ran: `limit_held` says whether the limit to the
    /// performance cores was set up and reached all of them (it does not matter for the other placements).
    pub fn line(&self, limit_held: bool) -> Cores {
        match self {
            Placement::Performance { cores, total } if limit_held => Cores::Performance {
                performance: cores.len(),
                total: *total,
            },
            Placement::Unpinned => Cores::Unpinned,
            Placement::Performance { .. } | Placement::Unknown => Cores::Unknown,
        }
    }
}

/// The limit of the engine processes to some logical cores. A process it cannot limit runs without it, and `failed`
/// says so afterwards (B4: «ese proceso [...] mide sin limitar»).
pub struct Pin {
    /// `None` only in the tests, to inject a limit that fails.
    limit: Option<Limit>,
    failed: AtomicBool,
}

/// The launcher and the affinity mask it limits itself to.
#[cfg(windows)]
struct Limit {
    launcher: PathBuf,
    mask: usize,
}
#[cfg(target_os = "linux")]
type Limit = nix::sched::CpuSet;
#[cfg(not(any(windows, target_os = "linux")))]
type Limit = ();

impl Pin {
    /// A limit to `cores`, through the benchmark's executable `launcher` on Windows; `None` when this system cannot
    /// set one up.
    pub fn new(cores: &[usize], launcher: &Path) -> Option<Pin> {
        Some(Pin {
            limit: Some(limit(cores, launcher)?),
            failed: AtomicBool::new(false),
        })
    }

    /// Starts `command`, with no input and its output and errors piped, limited to the cores; if the limit fails,
    /// the process runs without it. Its collected output goes through [`Pin::finish`] once it ends.
    pub fn spawn(&self, command: &mut Command) -> io::Result<Child> {
        let (child, limited) = match &self.limit {
            Some(limit) => spawn_limited(limit, command)?,
            None => (command.spawn()?, false),
        };
        if !limited {
            self.failed.store(true, Ordering::SeqCst);
        }
        Ok(child)
    }

    /// Takes the launcher's line out of the output of a process started by [`Pin::spawn`], and records a process
    /// that ran without the limit.
    pub fn finish(&self, stdout: &mut String) {
        if cfg!(windows) && self.limit.is_some() {
            let (first, rest) = stdout.split_once('\n').unwrap_or((stdout.as_str(), ""));
            if first.trim_end() != LIMITED {
                self.failed.store(true, Ordering::SeqCst);
            }
            *stdout = rest.to_string();
        }
    }

    /// Whether a process ran without the limit.
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::SeqCst)
    }
}

/// The launcher's role (`KRUMMI_BENCH_ROLE`).
pub const LAUNCHER_ROLE: &str = "limit";
/// The variable with the launcher's affinity mask, and the one with the role it gives the engine process.
const MASK_VARIABLE: &str = "KRUMMI_BENCH_LIMIT_MASK";
const INNER_ROLE_VARIABLE: &str = "KRUMMI_BENCH_LIMIT_ROLE";
/// The launcher's first line of output when it joined the job; `limit 0` when it could not.
const LIMITED: &str = "limit 1";

/// The launcher (Windows): joins a job limited to the mask of `KRUMMI_BENCH_LIMIT_MASK`, says on its first line of
/// output whether it could, and then runs `args` (a program and its arguments) with its own input and output, with
/// `KRUMMI_BENCH_ROLE` set to `KRUMMI_BENCH_LIMIT_ROLE` when there is one. Returns the program's exit code, or `1`
/// if it could not start. The job closes with the launcher and ends the processes in it, so stopping the launcher
/// stops the engine process too.
pub fn launcher_main(args: Vec<OsString>) -> i32 {
    use std::io::Write as _;
    let role = crate::worker::ROLE_VARIABLE;
    let job = launcher_job();
    let line = if job.is_some() { LIMITED } else { "limit 0" };
    let mut out = io::stdout();
    if writeln!(out, "{line}").and_then(|()| out.flush()).is_err() {
        return 1;
    }
    let Some((program, rest)) = args.split_first() else {
        eprintln!("falta el programa que lanzar");
        return 1;
    };
    let mut inner = Command::new(program);
    inner.args(rest).env_remove(role).env_remove(MASK_VARIABLE);
    if let Some(inner_role) = std::env::var_os(INNER_ROLE_VARIABLE) {
        inner.env(role, inner_role).env_remove(INNER_ROLE_VARIABLE);
    }
    let code = match inner.status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            eprintln!("{e}");
            1
        }
    };
    if let Some(job) = job {
        keep_until_exit(job);
    }
    code
}

/// The launcher is in its job too: closing the job now would end the launcher before it returns its code. The system
/// closes it when the launcher exits.
#[cfg(windows)]
fn keep_until_exit(job: win32job::Job) {
    std::mem::forget(job);
}

/// The job the launcher joined, `None` if it could not be set up or joined.
#[cfg(windows)]
fn launcher_job() -> Option<win32job::Job> {
    let mask: usize = std::env::var(MASK_VARIABLE).ok()?.parse().ok()?;
    let mut info = win32job::ExtendedLimitInfo::new();
    info.limit_affinity(mask).limit_kill_on_job_close();
    let job = win32job::Job::create_with_limit_info(&info).ok()?;
    job.assign_current_process().ok()?;
    Some(job)
}

/// Outside Windows there is no launcher job: the limit holds from the thread that starts the process.
#[cfg(not(windows))]
struct NoJob;

#[cfg(not(windows))]
fn launcher_job() -> Option<NoJob> {
    None
}

#[cfg(not(windows))]
fn keep_until_exit(_: NoJob) {}

/// The command that runs `command` through the launcher: the same program, arguments, folder and variables, with
/// the launcher's role and mask.
#[cfg(any(windows, test))]
fn through_launcher(launcher: &Path, mask: usize, command: &Command) -> Command {
    let role = crate::worker::ROLE_VARIABLE;
    let mut wrapped = Command::new(launcher);
    wrapped.arg(command.get_program()).args(command.get_args());
    for (name, value) in command.get_envs() {
        match value {
            Some(value) if name == role => wrapped.env(INNER_ROLE_VARIABLE, value),
            Some(value) => wrapped.env(name, value),
            None => wrapped.env_remove(name),
        };
    }
    if let Some(folder) = command.get_current_dir() {
        wrapped.current_dir(folder);
    }
    wrapped
        .env(role, LAUNCHER_ROLE)
        .env(MASK_VARIABLE, mask.to_string());
    wrapped
}

#[cfg(windows)]
fn limit(cores: &[usize], launcher: &Path) -> Option<Limit> {
    // A job's affinity is a mask of the first processor group, as the benchmark's own processes use. The `cores`
    // command numbers a core as `group * 64 + index` (voron/Cores.cs), so a core of another group has no bit here:
    // the limit cannot be set up, and the run measures without it (`Núcleos: desconocido`).
    let mut mask = 0usize;
    for &core in cores {
        mask |= 1usize.checked_shl(u32::try_from(core).ok()?)?;
    }
    Some(Limit {
        launcher: launcher.to_path_buf(),
        mask,
    })
}

/// Whether the process is limited is known when it ends, from the launcher's first line ([`Pin::finish`]).
#[cfg(windows)]
fn spawn_limited(limit: &Limit, command: &mut Command) -> io::Result<(Child, bool)> {
    use std::process::Stdio;
    let child = through_launcher(&limit.launcher, limit.mask, command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    Ok((child, true))
}

#[cfg(target_os = "linux")]
fn limit(cores: &[usize], _: &Path) -> Option<Limit> {
    let mut set = nix::sched::CpuSet::new();
    for &core in cores {
        set.set(core).ok()?;
    }
    Some(set)
}

#[cfg(target_os = "linux")]
fn spawn_limited(set: &Limit, command: &mut Command) -> io::Result<(Child, bool)> {
    use nix::sched::sched_setaffinity;
    use nix::unistd::Pid;
    // The child inherits the affinity of the thread that starts it; this thread ends with the start, so no other
    // thread of the benchmark changes.
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let limited = sched_setaffinity(Pid::from_raw(0), set).is_ok();
                command.spawn().map(|child| (child, limited))
            })
            .join()
            .expect("the thread that starts a process does not panic")
    })
}

#[cfg(not(any(windows, target_os = "linux")))]
fn limit(_: &[usize], _: &Path) -> Option<Limit> {
    None
}

#[cfg(not(any(windows, target_os = "linux")))]
fn spawn_limited(_: &Limit, command: &mut Command) -> io::Result<(Child, bool)> {
    command.spawn().map(|child| (child, false))
}

#[cfg(test)]
impl Pin {
    /// A limit that fails for every process (AC-B3, an injected failure).
    pub fn failing() -> Pin {
        Pin {
            limit: None,
            failed: AtomicBool::new(false),
        }
    }
}

#[cfg(test)]
mod tests;
