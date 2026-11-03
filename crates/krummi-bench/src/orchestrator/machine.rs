//! What the report says of the machine and of the versions it ran (PRD, *Salida*). Everything is best effort:
//! what cannot be obtained is `None`, which the report shows as `desconocido`.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System};

use crate::report::Machine;

/// The root of the working copy whose Krummi the benchmark measures (K1).
const REPOSITORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// The machine, with the disk of the work folder `work`.
pub fn machine(work: &Path) -> Machine {
    let system = System::new_with_specifics(
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::nothing())
            .with_memory(MemoryRefreshKind::nothing().with_ram()),
    );
    let cpus = system.cpus();
    let (disk_model, file_system) = disk(work);
    Machine {
        system: operating_system(),
        cpu_model: cpus
            .first()
            .map(|c| c.brand().trim().to_string())
            .filter(|b| !b.is_empty()),
        logical_cores: (!cpus.is_empty()).then_some(cpus.len()),
        memory_bytes: Some(system.total_memory()).filter(|&m| m > 0),
        disk_model,
        file_system,
    }
}

/// The name and version of the operating system; on Linux, the distribution with its version and the kernel's.
fn operating_system() -> Option<String> {
    if cfg!(windows) {
        // `System::long_os_version` is like `Windows 11 Pro`; `ver` gives the version, like `10.0.26200.6901`.
        let name = System::long_os_version()?;
        let version = output("cmd", ["/c", "ver"], None).and_then(|ver| {
            let inside = ver.split('[').nth(1)?.split(']').next()?;
            let number = inside.split_whitespace().last()?;
            Some(number.split('.').take(3).collect::<Vec<_>>().join("."))
        });
        Some(match version {
            Some(v) => format!("{name} {v}"),
            None => name,
        })
    } else {
        let name = System::name()?;
        let distribution = match System::os_version() {
            Some(v) => format!("{name} {v}"),
            None => name,
        };
        Some(match System::kernel_version() {
            Some(k) => format!("{distribution} (Linux {k})"),
            None => distribution,
        })
    }
}

/// The model and the file system of the disk that holds `work`.
fn disk(work: &Path) -> (Option<String>, Option<String>) {
    let Some(path) = fs::canonicalize(work).ok().map(without_verbatim_prefix) else {
        return (None, None);
    };
    let disks = Disks::new_with_refreshed_list();
    let Some(disk) = disks
        .list()
        .iter()
        .filter(|d| path.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
    else {
        return (None, None);
    };
    let file_system =
        Some(disk.file_system().to_string_lossy().trim().to_string()).filter(|f| !f.is_empty());
    let model = if cfg!(windows) {
        let letter = disk
            .mount_point()
            .to_string_lossy()
            .chars()
            .next()
            .filter(char::is_ascii_alphabetic);
        letter.and_then(|l| {
            let script = format!("(Get-Partition -DriveLetter {l} | Get-Disk).FriendlyName");
            output(
                "powershell",
                ["-NoProfile", "-NonInteractive", "-Command", &script],
                None,
            )
        })
    } else {
        // `-s` lists the device and then the disk it is on; the model is on the disk's line.
        let device = disk.name().to_string_lossy().into_owned();
        output("lsblk", ["-sn", "-o", "MODEL", &device], None).and_then(|o| {
            o.lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(String::from)
        })
    };
    (model, file_system)
}

/// `\\?\C:\x` as `C:\x`, so that it compares with the mount points of `Disks`.
fn without_verbatim_prefix(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => path,
    }
}

/// The Rust compiler that builds the working copy, such as `1.99.0`.
pub fn rust() -> Option<String> {
    let version = output("rustc", ["--version"], Some(Path::new(REPOSITORY)))?;
    version.split_whitespace().nth(1).map(String::from)
}

/// The first 12 characters of the last commit of the working copy, and whether tracked files have changes.
pub fn krummi_commit() -> (Option<String>, bool) {
    let repository = Path::new(REPOSITORY);
    let commit = output("git", ["rev-parse", "HEAD"], Some(repository))
        .filter(|c| c.len() >= 12)
        .map(|c| c[..12].to_string());
    let dirty = commit.is_some()
        && output(
            "git",
            ["status", "--porcelain", "--untracked-files=no"],
            Some(repository),
        )
        .is_some();
    (commit, dirty)
}

/// The standard output of a short command that succeeded, trimmed, when it is not empty.
fn output<const N: usize>(program: &str, args: [&str; N], cwd: Option<&Path>) -> Option<String> {
    let mut command = Command::new(program);
    command
        .args(args.iter().map(OsStr::new))
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let out = command.output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !text.is_empty()).then_some(text)
}
