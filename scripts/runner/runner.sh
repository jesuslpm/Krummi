#!/usr/bin/env bash
# Runs one task of Krummi on this Linux machine for a development machine elsewhere (docs/linux-runner.md).
# Invoke-Linux.ps1 copies this script here, pushes the commit under test to refs/runner/<sha> and calls it.
#
#   runner.sh init
#   runner.sh <check|build|bench|run> <commit sha> [-- arguments...]
#
# The arguments are shell text that the task's own shell reads, so they may use $KRUMMI_RESULTS, quotes and pipes.
#
#   check  cargo fmt --check, clippy and the test suite (no arguments)
#   build  cargo build --release --workspace [arguments]
#   bench  cargo run --release -p krummi-bench -- [arguments]
#   run    the arguments as a command (examples, perf record, anything else)
#
# Everything lives under $KRUMMI_RUNNER_HOME (default ~/krummi-runner): repo.git (receives the pushes), src (the
# checkout, reset on every run), target (build cache) and results/<run> (output.log, meta.txt and whatever the task
# writes to $KRUMMI_RESULTS). One run at a time, so that measurements do not disturb each other.
# Exit status: the task's; 2 usage error; 3 another run is in progress.
set -euo pipefail

# In braces so that bash reads the whole script before running it: Invoke-Linux.ps1 may copy it again meanwhile.
{
usage() { echo "uso: runner.sh init | runner.sh <check|build|bench|run> <sha del commit> [-- argumentos...]" >&2; exit 2; }

home="${KRUMMI_RUNNER_HOME:-$HOME/krummi-runner}"
task="${1:-}"
mkdir -p "$home"
# Its own lock, which waits: two first calls must not initialize at once, and a call made during a run must not fail here.
(
    flock 8
    [ -d "$home/repo.git" ] || git init -q --bare "$home/repo.git"
) 8>"$home/init.lock"
[ "$task" = init ] && exit 0

case "$task" in check | build | bench | run) ;; *) usage ;; esac
[ $# -ge 2 ] || usage
commit="$2"
shift 2
if [ "${1:-}" = -- ]; then shift; fi

arguments="$*"
case "$task" in
    check)
        [ -z "$arguments" ] || usage
        line='cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace --no-fail-fast'
        ;;
    build) line="cargo build --release --workspace $arguments" ;;
    bench) line="cargo run --release -p krummi-bench -- $arguments" ;;
    run)
        [ -n "$arguments" ] || usage
        line="$arguments"
        ;;
esac
command=(bash -o pipefail -c "$line")

exec 9>"$home/lock"
flock -n 9 || { echo "runner: hay otra ejecución en curso" >&2; exit 3; }

# A non-interactive SSH session does not read the profile that puts cargo on PATH.
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="$home/target"

[ -d "$home/src/.git" ] || git init -q "$home/src"
cd "$home/src"
git fetch -q "$home/repo.git" '+refs/runner/*:refs/runner/*'
git checkout -q --force --detach "$commit"
# Twice -f: a repository that a task left inside the checkout goes too.
git clean -qffdx

started="$(date -u +%Y%m%dT%H%M%S.%3NZ)"
export KRUMMI_RESULTS="$home/results/$started-${commit:0:12}-$task"
mkdir -p "$KRUMMI_RESULTS"
{
    echo "commit=$commit"
    echo "task=$task"
    printf 'command='; printf '%q ' "${command[@]}"; echo
    echo "host=$(uname -n)"
    echo "kernel=$(uname -r)"
    echo "cpu=$(sed -n 's/^model name[^:]*: //p' /proc/cpuinfo | head -n 1)"
    echo "governor=$(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null || echo unknown)"
    echo "rustc=$(rustc --version 2>/dev/null || echo unknown)"
    echo "started=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >"$KRUMMI_RESULTS/meta.txt"

set +e
# tee -p: a dropped SSH connection must not kill the run; its log stays here.
# 9>&-: a process the task leaves behind must not keep the lock.
"${command[@]}" 9>&- 2>&1 | tee -p "$KRUMMI_RESULTS/output.log"
status=${PIPESTATUS[0]}
set -e

{
    echo "ended=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "exit=$status"
} >>"$KRUMMI_RESULTS/meta.txt"
# The newline first: the task's output may not end in one, and the caller reads this line.
printf '\nRESULTS_DIR=%s\n' "$KRUMMI_RESULTS"
exit "$status"
}
