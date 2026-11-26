# Linux runner: develop on Windows, build and measure on Linux

Development happens on the Windows machine. A Linux machine only runs what Windows sends it: builds, tests,
benchmarks and profiling of a **commit**. Nothing is edited there, and uncommitted changes never travel.

`scripts/runner/Invoke-Linux.ps1` copies `runner.sh` to the Linux machine over SSH, pushes the commit to a bare
repository there (nothing goes to GitHub), runs the task and copies its results back to `work/runner/<run>/`.
Because the script travels with each call, the commit under test does not need to contain it.

## Setup

On Linux, once:

1. Install the SSH server (`sudo apt install openssh-server`), authorize the Windows public key in
   `~/.ssh/authorized_keys` and set `PasswordAuthentication no` in `/etc/ssh/sshd_config.d/`.
2. Install what the tasks need: git, Rust with rustfmt and clippy, `perf` for profiling, and for the comparative
   benchmark the .NET 10 SDK and the RavenDB 7.2.7 sources, for example in `~/krummi-runner/deps/ravendb-7.2.7`
   (see the README, *Comparative benchmark*).

On Windows, once: PowerShell 7.3 or later, and `%USERPROFILE%\.krummi-runner.json`, which is personal and never
versioned:

```json
{ "destination": "user@host", "identityFile": "~/.ssh/krummi_runner" }
```

The host is an address or a name. Reserve the address in the router, or use the `<hostname>.local` name that the
Linux machine publishes. Connect once by hand (`ssh -i <key> user@host`) to accept the host key: the script never
prompts.

## Use

```powershell
pwsh scripts/runner/Invoke-Linux.ps1 check                      # fmt, clippy and tests of HEAD
pwsh scripts/runner/Invoke-Linux.ps1 build -Commit main         # release build of another commit
pwsh scripts/runner/Invoke-Linux.ps1 bench -Arguments '--ravendb $HOME/krummi-runner/deps/ravendb-7.2.7 --informe $KRUMMI_RESULTS/banco.md'
pwsh scripts/runner/Invoke-Linux.ps1 run 'cargo run --release -p krummi --example tree_timing'
pwsh scripts/runner/Invoke-Linux.ps1 run 'perf record -g -o $KRUMMI_RESULTS/perf.data -- cargo run --release -p krummi --example tree_timing'
```

- The arguments are one string of shell text, read on Linux by the task's shell, where `$KRUMMI_RESULTS` is set;
  quotes and pipes work. Use single quotes in PowerShell so that it reaches Linux unexpanded, and pass them with
  `-Arguments` when they start with a dash. `check` takes none.
- Write anything worth keeping to `$KRUMMI_RESULTS`: the checkout is wiped before each run.
- Each run leaves `output.log` and `meta.txt` (commit, command, kernel, CPU, governor, compiler, times, exit
  status). The script returns the task's exit status; 3 means that another run is in progress.
- Only one run at a time. For measurements, keep the Linux machine otherwise idle.
- If the connection drops, the run may finish anyway; its results stay in `~/krummi-runner/results/`.
- `perf` needs `kernel.perf_event_paranoid` of 1 or lower to sample your own processes with kernel frames.
- Everything on Linux lives in `~/krummi-runner/`; the folder can be deleted to start over.

## Tests

On Linux (the second needs `pwsh`):

```bash
bash scripts/runner/tests/test_runner.sh
bash scripts/runner/tests/Test-InvokeLinux.sh
```
