"""Tests for the worktree guard. Run: python -m unittest discover -s .claude/hooks

Every test goes through the hook's public interface: a PreToolUse JSON on stdin, CLAUDE_PROJECT_DIR in the
environment, and the exit code (2 blocks) plus stderr. The layout is a real repository with real `git worktree add`
copies, so the copy files (`.git` file, `worktrees/<id>/gitdir`, `HEAD`) and the recorded base
(`branch.<branch>.worktreebase`) are exactly what the worktree scripts produce.
"""

import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest

HOOKS = os.path.dirname(os.path.abspath(__file__))
GUARD = os.path.join(HOOKS, "worktree_guard.py")
SETTINGS = os.path.join(HOOKS, "..", "settings.json")
GIT_BASH = next(
    (p for p in (r"C:\Program Files\Git\bin\bash.exe", "/usr/bin/bash", "/bin/bash") if os.path.exists(p)), None
)


def write_block(copy):
    return f"Bloqueado: esta sesión trabaja en '{copy}' y no escribe fuera de ella."


def base_block(base):
    return f"Bloqueado: la base '{base}' solo avanza con la fusión del worktree."


def payload(tool, cwd, **tool_input):
    return {
        "session_id": "test",
        "cwd": cwd,
        "hook_event_name": "PreToolUse",
        "tool_name": tool,
        "tool_input": tool_input,
    }


def run_guard(project_dir, data, path=None):
    """Runs the guard like Claude Code does; `data` is a dict (sent as JSON) or raw text. `path` replaces PATH."""
    raw = data if isinstance(data, str) else json.dumps(data)
    env = dict(os.environ)
    env["CLAUDE_PROJECT_DIR"] = project_dir
    if path is not None:
        env["PATH"] = path
    result = subprocess.run(
        [sys.executable, "-I", GUARD],
        input=raw.encode("utf-8"),
        capture_output=True,
        env=env,
        cwd=project_dir,
        timeout=60,
    )
    return result.returncode, result.stderr.decode("utf-8", errors="replace").strip()


def git(*args, cwd=None):
    env = dict(os.environ)
    env["GIT_CONFIG_NOSYSTEM"] = "1"
    env["GIT_CONFIG_GLOBAL"] = os.devnull
    subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false", *args],
        cwd=cwd,
        env=env,
        check=True,
        capture_output=True,
    )


def remove_tree(path):
    for folder, _, files in os.walk(path):
        for name in files:
            try:
                os.chmod(os.path.join(folder, name), stat.S_IWRITE)
            except OSError:
                pass
    shutil.rmtree(path, ignore_errors=True)


def bash_spelling(path):
    """C:\\x\\y -> /c/x/y (Git Bash)."""
    drive, rest = os.path.splitdrive(path)
    return "/" + drive[0].lower() + rest.replace("\\", "/")


class GuardTestCase(unittest.TestCase):
    """A real repository: the main copy, copy a (base main), copy b (base `ensayo`, a rehearsal base), copy c
    (whose HEAD is `main`, which cannot normally happen) and a folder whose name only shares a prefix with the main
    copy's."""

    @classmethod
    def setUpClass(cls):
        cls.root = os.path.realpath(tempfile.mkdtemp(prefix="wt-guard-"))
        cls.main = os.path.join(cls.root, "krummi")
        cls.copy_a = os.path.join(cls.root, "wt", "krummi-a")
        cls.copy_b = os.path.join(cls.root, "wt", "krummi-b")
        cls.copy_c = os.path.join(cls.root, "wt", "krummi-c")
        cls.prefix_sibling = os.path.join(cls.root, "krummi-other")
        os.makedirs(cls.prefix_sibling)
        git("init", "-q", "-b", "main", cls.main)
        git("commit", "-q", "--allow-empty", "-m", "init", cwd=cls.main)
        git("branch", "ensayo", cwd=cls.main)
        git("worktree", "add", "-q", "-b", "feat/a", cls.copy_a, "main", cwd=cls.main)
        git("worktree", "add", "-q", "-b", "feat/b", cls.copy_b, "ensayo", cwd=cls.main)
        git("worktree", "add", "-q", "--detach", cls.copy_c, "main", cwd=cls.main)
        git("config", "branch.feat/a.worktreebase", "main", cwd=cls.main)
        git("config", "branch.feat/b.worktreebase", "ensayo", cwd=cls.main)
        with open(os.path.join(cls.main, ".git", "worktrees", "krummi-c", "HEAD"), "w") as head:
            head.write("ref: refs/heads/main\n")

    @classmethod
    def tearDownClass(cls):
        remove_tree(cls.root)

    def bash(self, copy, command):
        return run_guard(copy, payload("Bash", copy, command=command))

    def powershell(self, copy, command):
        return run_guard(copy, payload("PowerShell", copy, command=command))

    def write(self, copy, file_path):
        return run_guard(copy, payload("Write", copy, file_path=file_path, content="x"))

    def assertBlocked(self, outcome, message):
        self.assertEqual(outcome, (2, message))

    def assertAllowed(self, outcome):
        self.assertEqual(outcome, (0, ""))


class MainCopyTests(GuardTestCase):
    def test_main_copy_never_blocks(self):
        for outcome in (
            self.write(self.main, os.path.join(self.copy_a, "src", "x.rs")),
            self.bash(self.main, f"touch {self.copy_b}\\x"),
            self.bash(self.main, "git checkout ensayo"),
            self.bash(self.main, "git update-ref refs/heads/main HEAD"),
            self.powershell(self.main, f"Remove-Item {self.copy_a}\\work -Recurse"),
        ):
            self.assertAllowed(outcome)


class FileToolTests(GuardTestCase):
    def test_edits_inside_the_copy_pass(self):
        self.assertAllowed(self.write(self.copy_a, os.path.join(self.copy_a, "src", "x.rs")))
        self.assertAllowed(
            run_guard(
                self.copy_a,
                payload("Edit", self.copy_a, file_path=os.path.join(self.copy_a, "README.md"), old_string="a",
                        new_string="b"),
            )
        )
        self.assertAllowed(self.write(self.copy_a, "src/relative.rs"))

    def test_write_into_the_main_copy_is_blocked(self):
        self.assertBlocked(self.write(self.copy_a, os.path.join(self.main, "src", "x.rs")), write_block(self.copy_a))

    def test_edit_into_the_main_copy_is_blocked(self):
        outcome = run_guard(
            self.copy_a,
            payload("Edit", self.copy_a, file_path=os.path.join(self.main, "CLAUDE.md"), old_string="a",
                    new_string="b"),
        )
        self.assertBlocked(outcome, write_block(self.copy_a))

    def test_write_into_another_copy_is_blocked(self):
        self.assertBlocked(self.write(self.copy_a, os.path.join(self.copy_b, "x.md")), write_block(self.copy_a))

    def test_notebook_edit_into_another_copy_is_blocked(self):
        outcome = run_guard(
            self.copy_a,
            payload("NotebookEdit", self.copy_a, notebook_path=os.path.join(self.copy_b, "n.ipynb"), new_source="x"),
        )
        self.assertBlocked(outcome, write_block(self.copy_a))

    def test_relative_path_escaping_into_the_main_copy_is_blocked(self):
        self.assertBlocked(self.write(self.copy_a, "../../krummi/x.md"), write_block(self.copy_a))

    def test_paths_elsewhere_pass(self):
        self.assertAllowed(self.write(self.copy_a, os.path.join(self.root, "scratch", "notes.txt")))
        self.assertAllowed(self.write(self.copy_a, os.path.join(tempfile.gettempdir(), "x.txt")))

    def test_folder_sharing_only_a_prefix_with_the_main_copy_passes(self):
        self.assertAllowed(self.write(self.copy_a, os.path.join(self.prefix_sibling, "x.md")))
        self.assertAllowed(self.bash(self.copy_a, f"touch {self.prefix_sibling}\\x.md"))

    def test_path_spelling_variants_are_blocked(self):
        target = os.path.join(self.main, "src", "x.rs")
        for spelling in (
            target.replace("\\", "/"),
            target.lower(),
            target.upper(),
            target[0].lower() + target[1:].replace("\\", "/"),
            bash_spelling(target),
        ):
            with self.subTest(spelling=spelling):
                self.assertBlocked(self.write(self.copy_a, spelling), write_block(self.copy_a))


class ShellWriteTests(GuardTestCase):
    def test_shell_write_naming_the_main_copy_is_blocked(self):
        for command in (
            f"touch {self.main}\\x.md",
            f"cd {self.main} && cargo build",
            f'cargo build --manifest-path "{self.main}\\Cargo.toml"',
            f"cat notes.md > {self.main}\\notes.md",
            f"rg -l foo {self.main} | xargs rm",
            f"find {self.main} -name '*.tmp' -delete",
            f"git -C {self.main} commit -m x",
        ):
            with self.subTest(command=command):
                self.assertBlocked(self.bash(self.copy_a, command), write_block(self.copy_a))

    def test_powershell_write_naming_another_copy_is_blocked(self):
        self.assertBlocked(
            self.powershell(self.copy_a, f"Remove-Item -Recurse {self.copy_b}\\work"), write_block(self.copy_a)
        )

    def test_shell_path_spelling_variants_are_blocked(self):
        main = self.main
        for spelling in (main.replace("\\", "/"), main.lower(), main.replace("\\", "\\\\"), bash_spelling(main)):
            with self.subTest(spelling=spelling):
                self.assertBlocked(self.bash(self.copy_a, f"touch {spelling}/x.md"), write_block(self.copy_a))

    def test_read_only_commands_naming_the_main_copy_pass(self):
        main = self.main
        for command in (
            f"rg -n foo {main}",
            f"cat {main}\\README.md | head -5",
            f"ls {self.copy_b} && eza {main}",
            f"find {main} -name '*.md'",
            f"git -C {main} status",
            f"git -C {main} log --oneline -3",
            f"git -C {main} diff main..feat/a",
            f"git -C {main} branch --list",
            f"git -C {main} worktree list",
            f"git -C {main} config --get branch.feat/a.worktreebase",
            f"rg foo {main} > results.txt",
        ):
            with self.subTest(command=command):
                self.assertAllowed(self.bash(self.copy_a, command))
        self.assertAllowed(self.powershell(self.copy_a, f"Get-Content {main}\\README.md | Select-String foo"))
        self.assertAllowed(self.powershell(self.copy_a, f"Test-Path {main}\\x; Get-ChildItem {main}"))

    def test_commands_naming_only_the_current_copy_pass(self):
        self.assertAllowed(self.bash(self.copy_a, f"touch {self.copy_a}\\x.md && cargo build"))
        self.assertAllowed(self.powershell(self.copy_a, f"Remove-Item {self.copy_a}\\work\\out -Recurse"))


class BaseMoveTests(GuardTestCase):
    """Copy b has the rehearsal base `ensayo`; `main` is protected too."""

    def test_checkout_and_switch_to_the_base_are_blocked(self):
        for command in (
            "git checkout ensayo",
            "git switch ensayo",
            "git checkout -B ensayo",
            "git switch -C ensayo HEAD",
            f"git -C {self.copy_b} checkout ensayo",
            "git checkout ensayo > nul",
            "git checkout ensayo 2> /dev/null",
            "git switch ensayo >> log.txt 2>&1",
        ):
            with self.subTest(command=command):
                self.assertBlocked(self.bash(self.copy_b, command), base_block("ensayo"))

    def test_base_names_are_matched_without_regard_to_case(self):
        # Windows refs ignore case: `git checkout ENSAYO` checks out `ensayo` and later commits move it.
        for command, branch in (
            ("git checkout ENSAYO", "ensayo"),
            ("git branch -f Ensayo HEAD", "ensayo"),
            ("git push . HEAD:Main", "main"),
            ("git update-ref refs/heads/MAIN HEAD", "main"),
        ):
            with self.subTest(command=command):
                self.assertBlocked(self.bash(self.copy_b, command), base_block(branch))

    def test_the_rehearsal_base_stays_protected_when_git_cannot_run(self):
        no_git = os.path.dirname(sys.executable)
        outcome = run_guard(self.copy_b, payload("Bash", self.copy_b, command="git checkout ensayo"), path=no_git)
        self.assertBlocked(outcome, base_block("ensayo"))

    def test_branch_forms_naming_the_base_are_blocked(self):
        for command in (
            "git branch -f ensayo HEAD",
            "git branch -d ensayo",
            "git branch -D ensayo",
            "git branch -m ensayo old",
            "git branch -M feat/b ensayo",
        ):
            with self.subTest(command=command):
                self.assertBlocked(self.bash(self.copy_b, command), base_block("ensayo"))

    def test_update_ref_of_the_base_is_blocked(self):
        self.assertBlocked(self.bash(self.copy_b, "git update-ref refs/heads/ensayo HEAD"), base_block("ensayo"))

    def test_push_and_fetch_into_the_base_are_blocked(self):
        for command in (
            "git push . HEAD:ensayo",
            "git push origin ensayo",
            "git push . +feat/b:refs/heads/ensayo",
            "git fetch . feat/b:ensayo",
            "git pull origin x:ensayo",
        ):
            with self.subTest(command=command):
                self.assertBlocked(self.bash(self.copy_b, command), base_block("ensayo"))

    def test_rebase_of_the_base_branch_is_blocked(self):
        self.assertBlocked(self.bash(self.copy_b, "git rebase feat/b ensayo"), base_block("ensayo"))

    def test_merge_reset_commit_rebase_on_a_protected_head_are_blocked(self):
        for command in ("git merge feat/a", "git reset --hard HEAD~1", "git commit -m x", "git rebase feat/a"):
            with self.subTest(command=command):
                self.assertBlocked(self.bash(self.copy_c, command), base_block("main"))

    def test_main_is_protected_when_the_base_is_a_rehearsal_base(self):
        for command in ("git checkout main", "git push . HEAD:main", "git update-ref refs/heads/main HEAD"):
            with self.subTest(command=command):
                self.assertBlocked(self.bash(self.copy_b, command), base_block("main"))

    def test_the_base_of_a_main_based_copy_is_main(self):
        self.assertBlocked(self.bash(self.copy_a, "git checkout main"), base_block("main"))
        self.assertBlocked(self.powershell(self.copy_a, "& git branch -f main HEAD"), base_block("main"))

    def test_git_commands_that_do_not_move_the_base_pass(self):
        for command in (
            "git merge ensayo",
            "git fetch origin ensayo",
            "git pull origin ensayo",
            "git checkout -b feat/x ensayo",
            "git checkout ensayo -- README.md",
            "git reset --hard ensayo",
            "git rebase ensayo",
            "git switch feat/b",
            "git commit -m 'feat: x'",
            "git log ensayo..HEAD",
            "git branch -f feat/b ensayo",
        ):
            with self.subTest(command=command):
                self.assertAllowed(self.bash(self.copy_b, command))

    def test_the_merge_script_passes(self):
        script = "powershell -NoProfile -ExecutionPolicy Bypass -File scripts/worktree/Merge-Worktree.ps1"
        self.assertAllowed(self.bash(self.copy_b, script))
        self.assertAllowed(self.powershell(self.copy_b, "& .\\scripts\\worktree\\Merge-Worktree.ps1"))


class UnexpectedInputTests(GuardTestCase):
    def test_unreadable_or_unexpected_input_never_blocks(self):
        for data in (
            "this is { not json",
            "",
            json.dumps({"tool_name": "Bash"}),
            json.dumps({"tool_name": "Bash", "tool_input": "git checkout main"}),
            json.dumps({"tool_name": "Write", "tool_input": {"file_path": 3}}),
            json.dumps(payload("Read", self.copy_a, file_path=os.path.join(self.main, "x"))),
            json.dumps([1, 2]),
        ):
            with self.subTest(data=data):
                self.assertAllowed(run_guard(self.copy_a, data))

    def test_copy_with_unreadable_git_file_never_blocks(self):
        folder = os.path.realpath(tempfile.mkdtemp(prefix="wt-guard-odd-"))
        try:
            with open(os.path.join(folder, ".git"), "w") as dot_git:
                dot_git.write("not a gitdir line\n")
            self.assertAllowed(self.bash(folder, f"touch {self.main}\\x && git checkout main"))
        finally:
            remove_tree(folder)


@unittest.skipUnless(GIT_BASH, "Git Bash not found")
class SettingsCommandTests(unittest.TestCase):
    """The pre-check registered in .claude/settings.json, run the way Claude Code runs it (Git Bash -c)."""

    def setUp(self):
        self.root = os.path.realpath(tempfile.mkdtemp(prefix="wt-guard-settings-"))
        self.main = os.path.join(self.root, "main")
        self.copy = os.path.join(self.root, "copy")
        admin = os.path.join(self.main, ".git", "worktrees", "copy")
        os.makedirs(admin)
        os.makedirs(self.copy)
        with open(os.path.join(admin, "gitdir"), "w") as f:
            f.write(self.copy.replace("\\", "/") + "/.git\n")
        with open(os.path.join(admin, "HEAD"), "w") as f:
            f.write("ref: refs/heads/feat/x\n")
        with open(os.path.join(self.copy, ".git"), "w") as f:
            f.write("gitdir: " + admin.replace("\\", "/") + "\n")
        for folder in (self.main, self.copy):
            os.makedirs(os.path.join(folder, ".claude", "hooks"))
            shutil.copy(GUARD, os.path.join(folder, ".claude", "hooks", "worktree_guard.py"))

    def tearDown(self):
        remove_tree(self.root)

    def hook_command(self):
        with open(SETTINGS, encoding="utf-8") as f:
            settings = json.load(f)
        (entry,) = settings["hooks"]["PreToolUse"]
        self.assertEqual(entry["matcher"], "Bash|PowerShell|Write|Edit|NotebookEdit")
        return entry["hooks"][0]["command"]

    def python3_only(self):
        """A folder whose only Python is a `python3` launcher, as on Linux without python-is-python3."""
        folder = os.path.join(self.root, "bin")
        os.makedirs(folder, exist_ok=True)
        launcher = os.path.join(folder, "python3")
        with open(launcher, "w", newline="\n") as f:
            f.write(f'#!/bin/sh\nexec "{sys.executable.replace(os.sep, "/")}" "$@"\n')
        os.chmod(launcher, os.stat(launcher).st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        return folder

    def run_command(self, project_dir, data, with_python, python3_only=False):
        path = [os.path.dirname(GIT_BASH)]
        if with_python:
            path.insert(0, os.path.dirname(sys.executable))
        if python3_only:
            path.insert(0, self.python3_only())
        env = {k: v for k, v in os.environ.items() if k.upper() not in ("PATH", "CLAUDE_PROJECT_DIR")}
        env["PATH"] = os.pathsep.join(path)
        env["CLAUDE_PROJECT_DIR"] = project_dir
        result = subprocess.run(
            [GIT_BASH, "-c", self.hook_command()],
            input=json.dumps(data).encode("utf-8"),
            capture_output=True,
            env=env,
            cwd=project_dir,
            timeout=60,
        )
        return result.returncode, result.stderr.decode("utf-8", errors="replace").strip()

    def test_main_copy_exits_0_without_running_python(self):
        data = payload("Write", self.main, file_path=os.path.join(self.copy, "x.md"), content="x")
        self.assertEqual(self.run_command(self.main, data, with_python=False), (0, ""))

    def test_copy_runs_the_guard(self):
        blocked = payload("Write", self.copy, file_path=os.path.join(self.main, "x.md"), content="x")
        allowed = payload("Write", self.copy, file_path=os.path.join(self.copy, "x.md"), content="x")
        self.assertEqual(self.run_command(self.copy, blocked, with_python=True), (2, write_block(self.copy)))
        self.assertEqual(self.run_command(self.copy, allowed, with_python=True), (0, ""))

    def test_copy_runs_the_guard_when_only_python3_exists(self):
        blocked = payload("Write", self.copy, file_path=os.path.join(self.main, "x.md"), content="x")
        result = self.run_command(self.copy, blocked, with_python=False, python3_only=True)
        self.assertEqual(result, (2, write_block(self.copy)))

    def test_copy_without_python_says_the_guard_did_not_run(self):
        data = payload("Write", self.copy, file_path=os.path.join(self.main, "x.md"), content="x")
        self.assertEqual(
            self.run_command(self.copy, data, with_python=False),
            (1, "Guarda de worktrees: no se encontró Python (python ni python3); la guarda no se aplicó."),
        )


if __name__ == "__main__":
    unittest.main()
