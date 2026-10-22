"""PreToolUse guard for sessions opened at the root of a worktree copy (PRD worktrees, W8).

In a copy it blocks:
- Write/Edit/NotebookEdit on a path inside the main copy or another copy;
- Bash/PowerShell commands that name the main copy or another copy, unless every segment of the command starts
  with a read-only command;
- git commands that move the copy's base or `main` by hand (only the worktree merge moves them).

It is a safety net against habit, not against an adversary: it reads the command as launched (not the scripts it
calls), and it never blocks input or a layout it cannot interpret. In the main copy (`.git` is a folder) it does
nothing; .claude/settings.json does not even start Python there. Exit 2 blocks the tool call and shows stderr to
the agent.

Layout: in a copy `<copy>/.git` is a file `gitdir: <main>/.git/worktrees/<id>`; the main copy is the parent of
`<main>/.git`; every other copy is read from `<main>/.git/worktrees/*/gitdir` (it points to `<copy>/.git`). The
copy's branch is in `<main>/.git/worktrees/<id>/HEAD` and its base in `git config branch.<branch>.worktreebase`.
"""

import json
import os
import re
import shlex
import subprocess
import sys

WRITE_MESSAGE = "Bloqueado: esta sesión trabaja en '{folder}' y no escribe fuera de ella."
BASE_MESSAGE = "Bloqueado: la base '{base}' solo avanza con la fusión del worktree."

SHELL_TOOLS = {"Bash", "PowerShell"}
FILE_TOOLS = {"Write": "file_path", "Edit": "file_path", "NotebookEdit": "notebook_path"}

# Commands that only read. PowerShell aliases of the listed cmdlets (gc, gci, dir, sls, type) count as them.
READ_ONLY = {
    "rg", "grep", "cat", "bat", "head", "tail", "ls", "eza", "fd", "find",
    "get-content", "gc", "type", "get-childitem", "gci", "dir", "select-string", "sls", "test-path",
}
FIND_WRITES = {"-delete", "-exec", "-execdir", "-ok", "-okdir", "-fprint", "-fprint0", "-fprintf", "-fls"}
FD_WRITES = {"-x", "--exec", "-X", "--exec-batch"}
GIT_READ_ONLY = {"log", "show", "diff", "status", "rev-parse"}
GIT_CONFIG_READS = {"--get", "--get-all", "--get-regexp", "--list", "-l"}
BRANCH_WRITES_SHORT = set("fdDmMC")
BRANCH_WRITES_LONG = {"--force", "--delete", "--move"}
# Commands that move the branch checked out in the copy.
GIT_MOVES_HEAD = {"merge", "reset", "commit", "rebase", "pull", "cherry-pick", "revert", "am"}
# git global options that take a separate value.
GIT_GLOBAL_WITH_VALUE = {"-C", "-c", "--git-dir", "--work-tree", "--namespace", "--config-env"}

SEGMENT_SPLIT = re.compile(r"\r?\n|;|&&|\|\||\|")
REDIRECT = re.compile(r"(?<![<>])[0-9*]?>>?(?!&)\s*(\"[^\"]*\"|'[^']*'|[^\s;|&<>]+)")


# ---- Paths -----------------------------------------------------------------------------------------------------


def to_windows(path):
    """Git Bash `/c/x` -> `c:/x`; anything else unchanged."""
    match = re.match(r"^/([A-Za-z])(/|$)", path)
    return f"{match.group(1)}:/{path[match.end():]}" if match else path


def canon(path):
    """Case-insensitive absolute spelling with backslashes, for comparisons."""
    return os.path.normcase(os.path.normpath(to_windows(path)))


def spellings(root):
    """Every spelling of an existing root: as recorded and fully resolved (8.3 names, links)."""
    names = {canon(root)}
    try:
        names.add(canon(os.path.realpath(root)))
    except (OSError, ValueError):
        pass
    return names


def owner(path, roots):
    """The root (key of `roots`) that contains `path`, the deepest one; None when none does."""
    best, best_len = None, -1
    for key, names in roots.items():
        for name in names:
            if (path == name or path.startswith(name.rstrip("\\") + "\\")) and len(name) > best_len:
                best, best_len = key, len(name)
    return best


def text_form(text):
    """Lower-case text with `/` separators and Git Bash drives as `c:/`, to find paths in a command."""
    text = re.sub(r"[\\/]+", "/", text).lower()
    return re.sub(r"(?<![\w.:~$-])/([a-z])/", r"\1:/", text)


# ---- Layout ----------------------------------------------------------------------------------------------------


def read_text(path):
    with open(path, encoding="utf-8-sig") as f:
        return f.read().strip()


class Layout:
    """The current copy, the main copy, the other copies, the copy's branch and its base."""

    def __init__(self, copy):
        self.copy = os.path.normpath(copy)
        line = read_text(os.path.join(copy, ".git"))
        if not line.startswith("gitdir:"):
            raise ValueError("not a worktree .git file")
        admin = os.path.normpath(os.path.join(copy, to_windows(line[len("gitdir:"):].strip())))
        try:
            common = os.path.normpath(os.path.join(admin, read_text(os.path.join(admin, "commondir"))))
        except OSError:
            common = os.path.dirname(os.path.dirname(admin))
        self.main = os.path.dirname(common)
        self.common = common
        self.others = []
        worktrees = os.path.join(common, "worktrees")
        for entry in os.listdir(worktrees):
            try:
                gitdir = read_text(os.path.join(worktrees, entry, "gitdir"))
            except OSError:
                continue
            other = os.path.dirname(os.path.normpath(os.path.join(worktrees, entry, to_windows(gitdir))))
            if canon(other) not in spellings(self.copy):
                self.others.append(other)
        head = ""
        try:
            head = read_text(os.path.join(admin, "HEAD"))
        except OSError:
            pass
        self.branch = head[len("ref: refs/heads/"):] if head.startswith("ref: refs/heads/") else None
        self._protected = None
        self._roots = None

    def roots(self):
        """{"copy": spellings, "main": spellings, "other<i>": spellings}."""
        if self._roots is None:
            self._roots = {"copy": spellings(self.copy), "main": spellings(self.main)}
            for i, other in enumerate(self.others):
                self._roots[f"other{i}"] = spellings(other)
        return self._roots

    def base(self):
        if not self.branch:
            return None
        try:
            result = subprocess.run(
                ["git", "-C", self.copy, "config", "--get", f"branch.{self.branch}.worktreebase"],
                capture_output=True, text=True, timeout=5,
            )
            value = result.stdout.strip()
            if result.returncode == 0 and value:
                return value
        except (OSError, subprocess.SubprocessError):
            pass
        return self.base_from_config_file()

    def base_from_config_file(self):
        """Reads the recorded base straight from the shared config when git cannot run, so a rehearsal base stays
        protected; it understands the plain `[branch "<name>"]` / `worktreebase = <base>` lines git writes."""
        try:
            lines = read_text(os.path.join(self.common, "config")).splitlines()
        except OSError:
            return None
        section = f'[branch "{self.branch}"]'
        inside = False
        for line in lines:
            line = line.strip()
            if line.startswith("["):
                inside = line == section
            elif inside:
                key, _, value = line.partition("=")
                if key.strip().lower() == "worktreebase" and value.strip():
                    return value.strip()
        return None

    def protected(self):
        """The branches only the worktree merge may move: the base first, then `main`."""
        if self._protected is None:
            base = self.base()
            self._protected = [b for b in (base, "main") if b] if base != "main" else ["main"]
        return self._protected


# ---- File tools ------------------------------------------------------------------------------------------------


def file_target_outside(layout, path):
    target = canon(os.path.join(layout.copy, to_windows(path)))
    found = owner(target, layout.roots())
    return found is not None and found != "copy"


# ---- Shell commands --------------------------------------------------------------------------------------------


def tokens(segment):
    try:
        words = shlex.split(segment, posix=False)
    except ValueError:
        words = segment.split()
    out = []
    for word in words:
        if len(word) >= 2 and word[0] == word[-1] and word[0] in "\"'":
            word = word[1:-1]
        out.append(word)
    return out


def command_words(segment):
    """The words of a segment from its command on: no leading `(`, `{`, `&`, `@`, `!` or VAR=value."""
    words = tokens(segment.strip().lstrip("({&@! \t"))
    while words and re.match(r"^[A-Za-z_]\w*=", words[0]):
        words = words[1:]
    return words


def command_name(word):
    name = word.replace("\\", "/").rsplit("/", 1)[-1].lower()
    return name[:-4] if name.endswith(".exe") else name


def git_parts(words):
    """(subcommand, arguments) of a git command line, skipping global options; None if it is not git."""
    if not words or command_name(words[0]) != "git":
        return None
    i = 1
    while i < len(words) and words[i].startswith("-"):
        i += 2 if words[i] in GIT_GLOBAL_WITH_VALUE else 1
    if i >= len(words):
        return None
    return words[i].lower(), without_redirects(words[i + 1:])


def without_redirects(words):
    """Drops output redirections, attached (`>f`, `2>&1`) or spaced (`> f`), so their targets are not arguments."""
    out = []
    skip_target = False
    for word in words:
        if skip_target:
            skip_target = False
        elif re.fullmatch(r"[0-9*]?>>?", word):
            skip_target = True
        elif not re.match(r"^[0-9*]?>", word):
            out.append(word)
    return out


def git_is_read_only(sub, args):
    if sub in GIT_READ_ONLY:
        return not any(a.startswith("--output") for a in args)
    if sub == "worktree":
        return bool(args) and args[0] == "list"
    if sub == "config":
        return any(a in GIT_CONFIG_READS for a in args) or (bool(args) and args[0] in ("get", "list"))
    if sub == "branch":
        writes = any(
            a in BRANCH_WRITES_LONG or (re.match(r"^-[A-Za-z]+$", a) and set(a[1:]) & BRANCH_WRITES_SHORT)
            for a in args
        )
        return not writes and any(a in ("--list", "-l", "--show-current") for a in args)
    return False


def names_protected(text, layout):
    """Whether `text` names the main copy or another copy (a path-boundary match, the deepest root winning)."""
    form = text_form(text)
    roots = {key: {text_form(n) for n in names} for key, names in layout.roots().items()}
    for key, names in roots.items():
        if key == "copy":
            continue
        for name in names:
            for match in re.finditer(re.escape(name) + r"(?![\w.-])", form):
                deeper = any(
                    len(own) > len(name) and re.match(re.escape(own) + r"(?![\w.-])", form[match.start():])
                    for own in roots["copy"]
                )
                if not deeper:
                    return True
    return False


def segment_is_read_only(segment, layout):
    words = command_words(segment)
    if not words:
        return True
    for target in REDIRECT.findall(segment):
        if names_protected(target.strip("\"'"), layout):
            return False
    name = command_name(words[0])
    if name == "git":
        parts = git_parts(words)
        return parts is not None and git_is_read_only(*parts)
    if name == "find":
        return not any(w in FIND_WRITES for w in words)
    if name == "fd":
        return not any(w in FD_WRITES for w in words)
    return name in READ_ONLY


def ref_name(word):
    word = word.lstrip("+")
    return word[len("refs/heads/"):] if word.startswith("refs/heads/") else word


def positionals(args, with_value=()):
    """Non-option arguments before `--`, skipping the values of the options in `with_value`."""
    out, i = [], 0
    while i < len(args):
        arg = args[i]
        if arg == "--":
            break
        if arg in with_value:
            i += 2
            continue
        if not arg.startswith("-") or arg == "-":
            out.append(arg)
        i += 1
    return out


def option_value(args, names):
    for i, arg in enumerate(args):
        if arg in names and i + 1 < len(args):
            return args[i + 1]
        for name in names:
            if name.startswith("--") and arg.startswith(name + "="):
                return arg[len(name) + 1:]
    return None


def moved_branch(sub, args, layout):
    """The protected branch this git command would move, or None."""
    # Windows refs ignore case (`git checkout ENSAYO` checks out `ensayo`), so names are compared in lower case.
    protected = {name.lower(): name for name in layout.protected()}

    def hit(*names):
        for name in names:
            if name is not None and ref_name(name).lower() in protected:
                return protected[ref_name(name).lower()]
        return None

    if sub in GIT_MOVES_HEAD and layout.branch and layout.branch.lower() in protected:
        return protected[layout.branch.lower()]
    if sub == "checkout":
        new = option_value(args, ("-b", "-B", "--orphan"))
        if new is not None:
            return hit(new)
        pos = positionals(args, ("-b", "-B", "--orphan"))
        restores_files = "--" in args or len(pos) > 1
        return None if restores_files or not pos else hit(pos[0])
    if sub == "switch":
        new = option_value(args, ("-c", "-C", "--create", "--force-create", "--orphan"))
        if new is not None:
            return hit(new)
        pos = positionals(args)
        return hit(pos[0]) if pos else None
    if sub == "branch":
        flags = set()
        for arg in args:
            if re.match(r"^-[A-Za-z]+$", arg):
                flags |= set(arg[1:])
            elif arg in BRANCH_WRITES_LONG:
                flags.add({"--force": "f", "--delete": "d", "--move": "m"}[arg])
        pos = positionals(args)
        if flags & set("dDmM"):
            return hit(*pos)
        if "C" in flags:
            return hit(pos[-1]) if pos else None
        if "f" in flags:
            return hit(pos[0]) if pos else None
        return None
    if sub == "update-ref":
        for arg in positionals(args, ("-m",)):
            if arg.startswith("refs/heads/") and hit(arg):
                return hit(arg)
        return None
    if sub in ("push", "fetch", "pull"):
        pos = positionals(args, ("-o", "--push-option", "--receive-pack", "--upload-pack", "--repo", "--exec"))
        for spec in pos[1:]:
            if ":" in spec:
                found = hit(spec.split(":", 1)[1])
            else:
                found = hit(spec) if sub == "push" else None
            if found:
                return found
        return None
    if sub == "rebase":
        pos = positionals(args, ("--onto", "-s", "--strategy", "-X", "--strategy-option", "-x", "--exec"))
        return hit(pos[-1]) if len(pos) >= 2 else None
    return None


def check_shell(layout, command):
    segments = [s for s in SEGMENT_SPLIT.split(command) if s.strip()]
    for segment in segments:
        parts = git_parts(command_words(segment))
        if parts:
            branch = moved_branch(*parts, layout)
            if branch:
                return BASE_MESSAGE.format(base=branch)
    if names_protected(command, layout) and not all(segment_is_read_only(s, layout) for s in segments):
        return WRITE_MESSAGE.format(folder=layout.copy)
    return None


# ---- Entry point -----------------------------------------------------------------------------------------------


def check(copy, data):
    """The block message for a PreToolUse input in the session rooted at `copy`, or None."""
    if not os.path.isfile(os.path.join(copy, ".git")):
        return None
    if not isinstance(data, dict) or not isinstance(data.get("tool_input"), dict):
        return None
    tool, tool_input = data.get("tool_name"), data["tool_input"]
    if tool in SHELL_TOOLS:
        command = tool_input.get("command")
        if not isinstance(command, str) or not command.strip():
            return None
        return check_shell(Layout(copy), command)
    if tool in FILE_TOOLS:
        path = tool_input.get(FILE_TOOLS[tool])
        if not isinstance(path, str) or not path.strip():
            return None
        layout = Layout(copy)
        return WRITE_MESSAGE.format(folder=layout.copy) if file_target_outside(layout, path) else None
    return None


def main():
    try:
        copy = os.environ.get("CLAUDE_PROJECT_DIR") or os.getcwd()
        data = json.loads(sys.stdin.buffer.read().decode("utf-8-sig"))
        message = check(copy, data)
    except Exception:  # unreadable input or an unexpected layout never blocks
        return 0
    if message:
        sys.stderr.buffer.write((message + "\n").encode("utf-8"))
        sys.stderr.flush()
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
