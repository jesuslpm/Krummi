#!/usr/bin/env bash
# End to end test of Invoke-Linux.ps1 on Linux, with ssh and scp replaced by local stand-ins: the "remote" machine
# is a throwaway HOME here. Needs pwsh. Run: bash scripts/runner/tests/Test-InvokeLinux.sh
set -uo pipefail

scripts="$(cd "$(dirname "$0")/.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@t GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@t
export REMOTE_HOME="$scratch/remote"
unset KRUMMI_RUNNER_HOME
failures=0

check() { # <description> <command...>
    local what="$1"; shift
    if "$@" >/dev/null 2>&1; then echo "ok   $what"; else echo "FAIL $what"; failures=$((failures + 1)); fi
}

mkdir -p "$scratch/bin" "$REMOTE_HOME"
cat >"$scratch/bin/ssh" <<'SHIM'
#!/usr/bin/env bash
while [[ "$1" == -* ]]; do case "$1" in -i | -o) shift 2 ;; *) shift ;; esac; done
shift # destination
cd "$REMOTE_HOME" && HOME="$REMOTE_HOME" exec bash -c "$*"
SHIM
cat >"$scratch/bin/scp" <<'SHIM'
#!/usr/bin/env bash
while [[ "$1" == -* ]]; do case "$1" in -i | -o) shift 2 ;; *) shift ;; esac; done
local_path() { case "$1" in *@*:/*) echo "${1#*:}" ;; *@*:*) echo "$REMOTE_HOME/${1#*:}" ;; *) echo "$1" ;; esac; }
cp -r "$(local_path "$1")" "$(local_path "$2")"
SHIM
chmod +x "$scratch/bin/ssh" "$scratch/bin/scp"
export PATH="$scratch/bin:$PATH"

git init -q "$scratch/repo"
echo content >"$scratch/repo/file" && git -C "$scratch/repo" add file && git -C "$scratch/repo" commit -qm one
sha=$(git -C "$scratch/repo" rev-parse HEAD)
echo '{ "destination": "me@runner" }' >"$scratch/config.json"
export KRUMMI_RUNNER_CONFIG="$scratch/config.json"

invoke() { pwsh -NoProfile -File "$scripts/Invoke-Linux.ps1" "$@" -Repository "$scratch/repo" 2>&1; }

out=$(invoke run 'cat file'); status=$?
check "a run succeeds" test "$status" -eq 0
check "the output of the task is shown" grep -qx content <<<"$out"
check "the results are brought back" bash -c 'grep -qx "commit=$1" "$2"/work/runner/*/meta.txt' _ "$sha" "$scratch/repo"

rm -rf "$scratch/repo/work"
out=$(invoke run "printf '%s|' 'a b' \"it's\" '\$HOME' 'say \"hi\"'")
check "arguments arrive unchanged" grep -qF "a b|it's|\$HOME|say \"hi\"|" <<<"$out"

out=$(invoke run 'echo --reducido -p x')
check "arguments with dashes arrive" grep -qx -- "--reducido -p x" <<<"$out"
rm -rf "$scratch/repo/work"
invoke run -Arguments 'echo data >$KRUMMI_RESULTS/extra' >/dev/null
check "the results folder is known to the task" bash -c 'grep -qx data "$1"/work/runner/*/extra' _ "$scratch/repo"
rm -rf "$scratch/repo/work"

echo dirty >>"$scratch/repo/file"
out=$(invoke run 'cat file')
check "uncommitted changes are announced" grep -q "sin confirmar" <<<"$out"
check "uncommitted changes are not run" bash -c '! grep -qx dirty <<<"$1"' _ "$out"

invoke run "sh -c 'exit 7'" >/dev/null; status=$?
check "the exit status of the task is returned" test "$status" -eq 7

out=$(KRUMMI_RUNNER_CONFIG="$scratch/missing.json" invoke check); status=$?
check "a missing configuration fails" test "$status" -ne 0
check "a missing configuration says what to create" grep -q destination <<<"$out"

echo "failures: $failures"
[ "$failures" -eq 0 ]
