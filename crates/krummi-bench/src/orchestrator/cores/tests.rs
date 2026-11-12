use super::*;
use crate::orchestrator::child::{self, Ended};

// AC-B3: the classes the system gives decide the cores: the fastest class when there is more than one, none otherwise,
// with the *Núcleos* line of each case.
#[test]
fn the_fastest_class_is_chosen_only_when_there_are_several() {
    let same = [(0, 1), (1, 1), (2, 1)];
    assert_eq!(placement(Some(&same)), Placement::Unpinned);
    assert_eq!(placement(Some(&[])), Placement::Unpinned);
    assert_eq!(placement(None), Placement::Unknown);

    let two = [(3, 0), (0, 1), (1, 1), (2, 0)];
    assert_eq!(
        placement(Some(&two)),
        Placement::Performance {
            cores: vec![0, 1],
            total: 4
        }
    );
    // The classes are relative: the highest wins, whatever its value.
    let three = [(0, 2), (1, 7), (2, 5), (3, 7), (4, 2)];
    assert_eq!(
        placement(Some(&three)),
        Placement::Performance {
            cores: vec![1, 3],
            total: 5
        }
    );

    let performance = Cores::Performance {
        performance: 2,
        total: 5,
    };
    assert_eq!(placement(Some(&three)).line(true), performance);
    assert_eq!(placement(Some(&three)).line(false), Cores::Unknown);
    assert_eq!(placement(Some(&same)).line(true), Cores::Unpinned);
    assert_eq!(placement(Some(&[])).line(true), Cores::Unpinned);
    assert_eq!(placement(None).line(true), Cores::Unknown);
}

#[test]
fn the_classes_are_read_from_the_cores_command() {
    assert_eq!(
        parse_classes("core 0 1\ncore 16 0\n\n"),
        Some(vec![(0, 1), (16, 0)])
    );
    assert_eq!(parse_classes(""), Some(vec![]));
    for bad in ["core 0\n", "core x 1\n", "core 0 1\ncore 0 0\n", "hola\n"] {
        assert_eq!(parse_classes(bad), None, "{bad:?}");
    }
}

/// A command that prints its own affinity and then the one of a process it starts, one per line.
fn affinities() -> Command {
    if cfg!(windows) {
        let own = "[Diagnostics.Process]::GetCurrentProcess().ProcessorAffinity.ToInt64()";
        let mut command = Command::new("powershell");
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!("{own}; powershell -NoProfile -NonInteractive -Command '{own}'"),
        ]);
        command
    } else {
        let own = "grep Cpus_allowed_list /proc/self/status";
        let mut command = Command::new("sh");
        command.args(["-c", &format!("{own}; sh -c '{own}'")]);
        command
    }
}

/// The affinity of this process, as `affinities` prints it.
fn own_affinity() -> String {
    #[cfg(windows)]
    {
        let current = win32job::utils::get_current_process();
        win32job::utils::get_process_affinity_mask(current)
            .unwrap()
            .0
            .to_string()
    }
    #[cfg(not(windows))]
    {
        // The thread that runs the test, whose affinity the threads that start the processes inherit.
        std::fs::read_to_string("/proc/thread-self/status")
            .unwrap()
            .lines()
            .find(|l| l.starts_with("Cpus_allowed_list"))
            .unwrap()
            .to_string()
    }
}

/// The lowest logical core this test may run on, so that the test does not assume core 0 is allowed.
#[cfg(target_os = "linux")]
fn first_allowed_core() -> usize {
    let set = nix::sched::sched_getaffinity(nix::unistd::Pid::from_raw(0)).unwrap();
    (0..nix::sched::CpuSet::count())
        .find(|&c| set.is_set(c).unwrap_or(false))
        .unwrap()
}

// AC-B3 on Linux: a limited process and the process it starts run on the limit's cores; the benchmark's own affinity
// does not change. It limits to one core this test may use, so it runs on any machine. On Windows the limit goes
// through the benchmark's executable, the launcher, which `tests/cores.rs` runs.
#[cfg(target_os = "linux")]
#[test]
fn a_limited_process_and_the_ones_it_starts_run_on_its_cores() {
    let before = own_affinity();
    let core = first_allowed_core();
    let pin = Pin::new(&[core], Path::new("unused on Linux")).expect("a limit");
    let Ended::Finished(f) = child::run_with(&mut affinities(), Some(&pin)) else {
        panic!("the command runs");
    };
    assert_eq!(f.code, Some(0), "{}", f.stderr);
    let expected = format!("Cpus_allowed_list:	{core}");
    let lines: Vec<&str> = f.stdout.lines().map(str::trim_end).collect();
    assert_eq!(lines, [&*expected, &*expected], "{}", f.stdout);
    assert!(!pin.failed());
    assert_eq!(own_affinity(), before);

    // Without a limit, the process keeps the benchmark's affinity.
    let Ended::Finished(f) = child::run(&mut affinities()) else {
        panic!("the command runs");
    };
    assert_eq!(f.stdout.lines().next().map(str::trim_end), Some(&*before));
}

// R1 of the Codex code review: on Windows the engine process starts through the launcher, which joins the limited job
// before it creates the process, so the process is never outside the limit. The launcher's command keeps the
// program, its arguments, folder and variables, and passes the worker role on.
#[test]
fn the_launcher_runs_the_same_command_with_its_role_and_mask() {
    let role = crate::worker::ROLE_VARIABLE;
    let mut command = Command::new("worker.exe");
    command
        .args(["run", "krummi", "W1"])
        .env(role, "worker")
        .env("KRUMMI_BENCH_FAIL", "x")
        .env_remove("OTHER")
        .current_dir("folder");
    let wrapped = through_launcher(Path::new("bench.exe"), 0b1010, &command);
    assert_eq!(wrapped.get_program(), "bench.exe");
    let args: Vec<_> = wrapped.get_args().collect();
    assert_eq!(args, ["worker.exe", "run", "krummi", "W1"]);
    let envs: std::collections::BTreeMap<_, _> = wrapped.get_envs().collect();
    let value = |v: &str| Some(std::ffi::OsStr::new(v).to_os_string());
    let got = |k: &str| {
        envs.get(std::ffi::OsStr::new(k))
            .map(|v| v.map(|v| v.to_os_string()))
    };
    assert_eq!(got(role), Some(value(LAUNCHER_ROLE)));
    assert_eq!(got(MASK_VARIABLE), Some(value("10")));
    assert_eq!(got(INNER_ROLE_VARIABLE), Some(value("worker")));
    assert_eq!(got("KRUMMI_BENCH_FAIL"), Some(value("x")));
    assert_eq!(got("OTHER"), Some(None));
    assert_eq!(wrapped.get_current_dir(), Some(Path::new("folder")));
}

// The launcher's first line says whether it joined the job; it is not part of the worker's output.
#[cfg(windows)]
#[test]
fn the_launcher_line_is_taken_out_of_the_output() {
    let pin = Pin::new(&[0], Path::new("bench.exe")).expect("a limit");
    let mut out = "limit 1
rep 0 W1 1 1
"
    .to_string();
    pin.finish(&mut out);
    assert_eq!(
        out,
        "rep 0 W1 1 1
"
    );
    assert!(!pin.failed());
    let mut out = "limit 0
rep 0 W1 1 1
"
    .to_string();
    pin.finish(&mut out);
    assert_eq!(
        out,
        "rep 0 W1 1 1
"
    );
    assert!(pin.failed());
    // A launcher that failed before its line also counts as a process without the limit.
    let pin = Pin::new(&[0], Path::new("bench.exe")).expect("a limit");
    let mut out = String::new();
    pin.finish(&mut out);
    assert!(pin.failed());
}

// AC-B3: a process that cannot be limited still runs to its end, and the limit says so afterwards.
#[test]
fn a_process_that_cannot_be_limited_runs_without_the_limit() {
    let pin = Pin::failing();
    let Ended::Finished(f) = child::run_with(&mut affinities(), Some(&pin)) else {
        panic!("the command runs");
    };
    assert_eq!(f.code, Some(0), "{}", f.stderr);
    assert_eq!(
        f.stdout.lines().next().map(str::trim_end),
        Some(&*own_affinity())
    );
    assert!(pin.failed());
}
