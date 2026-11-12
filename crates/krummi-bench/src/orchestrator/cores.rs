//! The performance cores (B4 and B5 of the Krummi and Voron only PRD): which logical cores the engine processes run
//! on, and the limit that keeps them there. The classes come from the Voron program's `cores` command (one
//! `core <id> <class>` line per logical core, a higher class being a faster core). The limit uses safe APIs only
//! (K14): on Windows, a job object with an affinity limit, which the processes a limited process starts join too; on
//! Linux, the affinity of the thread that starts the process, which the process inherits from its creation.

use std::collections::BTreeSet;
use std::io;
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

#[cfg(windows)]
type Limit = win32job::Job;
#[cfg(target_os = "linux")]
type Limit = nix::sched::CpuSet;
#[cfg(not(any(windows, target_os = "linux")))]
type Limit = ();

impl Pin {
    /// A limit to `cores`; `None` when this system cannot set one up.
    pub fn new(cores: &[usize]) -> Option<Pin> {
        Some(Pin {
            limit: Some(limit(cores)?),
            failed: AtomicBool::new(false),
        })
    }

    /// Starts `command` limited to the cores; if the limit fails, the process runs without it.
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

    /// Whether a process ran without the limit.
    pub fn failed(&self) -> bool {
        self.failed.load(Ordering::SeqCst)
    }
}

#[cfg(windows)]
fn limit(cores: &[usize]) -> Option<Limit> {
    // A job's affinity is a mask of the first processor group, as the benchmark's own processes use. The `cores`
    // command numbers a core as `group * 64 + index` (voron/Cores.cs), so a core of another group has no bit here:
    // the limit cannot be set up, and the run measures without it (`Núcleos: desconocido`).
    let mut mask = 0usize;
    for &core in cores {
        mask |= 1usize.checked_shl(u32::try_from(core).ok()?)?;
    }
    let mut info = win32job::ExtendedLimitInfo::new();
    info.limit_affinity(mask);
    win32job::Job::create_with_limit_info(&info).ok()
}

#[cfg(windows)]
fn spawn_limited(job: &Limit, command: &mut Command) -> io::Result<(Child, bool)> {
    use std::os::windows::io::AsRawHandle as _;
    let child = command.spawn()?;
    let limited = job.assign_process(child.as_raw_handle() as isize).is_ok();
    Ok((child, limited))
}

#[cfg(target_os = "linux")]
fn limit(cores: &[usize]) -> Option<Limit> {
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
fn limit(_: &[usize]) -> Option<Limit> {
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
