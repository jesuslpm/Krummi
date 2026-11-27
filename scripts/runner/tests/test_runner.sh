#!/usr/bin/env bash
# Tests of runner.sh against a throwaway runner home and repository. Run: bash scripts/runner/tests/test_runner.sh
set -uo pipefail

runner="$(cd "$(dirname "$0")/.." && pwd)/runner.sh"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
export KRUMMI_RUNNER_HOME="$scratch/home"
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
failures=0

check() { # <description> <command...>
    local what="$1"; shift
    if "$@" >/dev/null 2>&1; then echo "ok   $what"; else echo "FAIL $what"; failures=$((failures + 1)); fi
}
results_of() { sed -n 's/^RESULTS_DIR=//p' <<<"$1"; }

bash "$runner" init >/dev/null
check "init creates the bare repository" git -C "$KRUMMI_RUNNER_HOME/repo.git" rev-parse --is-bare-repository

git init -q "$scratch/src"
echo one >"$scratch/src/file" && git -C "$scratch/src" add file && git -C "$scratch/src" commit -qm one
first=$(git -C "$scratch/src" rev-parse HEAD)
echo two >"$scratch/src/file" && git -C "$scratch/src" commit -qam two
second=$(git -C "$scratch/src" rev-parse HEAD)
for sha in "$first" "$second"; do
    git -C "$scratch/src" push -q "$KRUMMI_RUNNER_HOME/repo.git" "$sha:refs/runner/$sha"
done

out=$(bash "$runner" run "$first" -- 'cat file'); status=$?
dir=$(results_of "$out")
check "run succeeds" test "$status" -eq 0
check "run checks out the requested commit" grep -qx one "$dir/output.log"
check "meta records the commit" grep -qx "commit=$first" "$dir/meta.txt"
check "meta records the exit status" grep -qx "exit=0" "$dir/meta.txt"

out=$(bash "$runner" run "$second" -- 'cat file; touch leftover')
check "a later run switches commit" grep -qx two "$(results_of "$out")/output.log"
out=$(bash "$runner" run "$second" -- ls)
check "untracked files do not survive to the next run" bash -c '! grep -qx leftover "$1"' _ "$(results_of "$out")/output.log"

bash "$runner" run "$second" -- 'git init -q nested && touch nested/file' >/dev/null
out=$(bash "$runner" run "$second" -- ls)
check "nested repositories do not survive to the next run" bash -c '! grep -qx nested "$1"' _ "$(results_of "$out")/output.log"

out=$(bash "$runner" run "$second" -- "printf '%s|' 'a b' \"it's\" '\$HOME'"); dir=$(results_of "$out")
check "quoted arguments arrive unchanged" grep -qF "a b|it's|\$HOME|" "$dir/output.log"

out=$(bash "$runner" run "$second" -- 'sh -c "exit 7"'); status=$?
check "the exit status of the task is returned" test "$status" -eq 7
check "a failed run keeps its results" grep -qx "exit=7" "$(results_of "$out")/meta.txt"
bash "$runner" run "$second" -- 'false | true' >/dev/null
check "a failure inside a pipe fails the run" test "$?" -ne 0

out=$(bash "$runner" run "$second" -- 'echo data >$KRUMMI_RESULTS/extra')
check "the task sees the results folder" grep -qx data "$(results_of "$out")/extra"

bash "$runner" check "$second" -- '-p krummi' >/dev/null 2>&1
check "check rejects arguments instead of ignoring them" test "$?" -eq 2

mkdir -p "$KRUMMI_RUNNER_HOME"
exec 8>"$KRUMMI_RUNNER_HOME/lock"; flock -n 8
bash "$runner" run "$second" -- true >/dev/null 2>&1; status=$?
exec 8>&-
check "a second run is rejected while one is in progress" test "$status" -eq 3

bash "$runner" run 0000000000000000000000000000000000000000 -- true >/dev/null 2>&1
check "an unknown commit fails" test "$?" -ne 0
bash "$runner" nonsense "$second" >/dev/null 2>&1
check "an unknown task is a usage error" test "$?" -eq 2

# A commit whose CI pins a Rust version is run with that version, from a Rust installation of the runner's own, so
# that nothing added to the user's installation (rust-src, for one) changes its results.
mkdir -p "$scratch/user/.cargo/bin"
cat >"$scratch/user/.cargo/bin/rustup" <<'SHIM'
#!/usr/bin/env bash
echo "$RUSTUP_HOME $*" >>"$HOME/rustup.log"
[ "$1 $2" = "toolchain list" ] && cat "$RUSTUP_HOME/installed" 2>/dev/null
[ "$1 $2" = "toolchain install" ] && mkdir -p "$RUSTUP_HOME" && echo "$3-host" >>"$RUSTUP_HOME/installed"
exit 0
SHIM
chmod +x "$scratch/user/.cargo/bin/rustup"
mkdir -p "$scratch/src/.github/workflows"
printf 'env:\n  RUST_TOOLCHAIN: "1.2.3"\n' >"$scratch/src/.github/workflows/ci.yml"
git -C "$scratch/src" add .github && git -C "$scratch/src" commit -qm ci
pinned=$(git -C "$scratch/src" rev-parse HEAD)
git -C "$scratch/src" push -q "$KRUMMI_RUNNER_HOME/repo.git" "$pinned:refs/runner/$pinned"
show='echo "$RUSTUP_TOOLCHAIN|$RUSTUP_HOME"'
out=$(HOME="$scratch/user" bash "$runner" run "$pinned" -- "$show"); dir=$(results_of "$out")
check "the task uses the version of the commit's CI, from the runner's Rust" \
    grep -qxF "1.2.3|$KRUMMI_RUNNER_HOME/rustup" "$dir/output.log"
check "a missing version is installed there, with rustfmt and clippy" \
    grep -qxF "$KRUMMI_RUNNER_HOME/rustup toolchain install 1.2.3 --profile minimal --component rustfmt,clippy" "$scratch/user/rustup.log"
HOME="$scratch/user" bash "$runner" run "$pinned" -- true >/dev/null
check "an installed version is not installed again" test "$(grep -c 'toolchain install' "$scratch/user/rustup.log")" -eq 1
out=$(env -u RUSTUP_TOOLCHAIN -u RUSTUP_HOME bash "$runner" run "$second" -- "$show")
check "a commit without a pinned version keeps the user's Rust" grep -qxF '|' "$(results_of "$out")/output.log"

failed_inits=0
for attempt in $(seq 40); do
    KRUMMI_RUNNER_HOME="$scratch/cold$attempt" bash "$runner" init >/dev/null 2>&1 &
    first_init=$!
    KRUMMI_RUNNER_HOME="$scratch/cold$attempt" bash "$runner" init >/dev/null 2>&1 || failed_inits=$((failed_inits + 1))
    wait "$first_init" || failed_inits=$((failed_inits + 1))
done
check "simultaneous first calls both succeed" test "$failed_inits" -eq 0

echo "failures: $failures"
[ "$failures" -eq 0 ]
