"""Moves documents of the repository and rewrites every reference to them, so that no working link breaks.

Commands, run from anywhere inside the repository with `python -I scripts/docs/move_docs.py <command> ...`:

- ``archive <feature> <module>``: moves a finished execution, every ``odd/tasks/<feature>.md``,
  ``<feature>-report.md``, ``<feature>-phases.tsv`` and ``<feature>-measurements.md`` that exists, to
  ``odd/done/<module>/``.
- ``reopen <feature>``: moves an archived execution back, from ``odd/done/<module>/`` to ``odd/tasks/``, to work on it
  again.
- ``move <manifest>``: moves the files a manifest lists, one ``<old path>\\t<new path>`` per line, paths from the
  repository root; blank lines and lines starting with ``#`` are skipped. A relative manifest path is taken from the
  current folder.
- ``check``: lists the relative Markdown links of tracked Markdown and Rust files whose target does not exist, one
  ``<file>:<line>: <target>`` per line, and exits with 1 when there is any.

A move rewrites, in every tracked text file:

- relative Markdown links, inline (``[text](path#anchor)``) and reference definitions (``[id]: path``, also after
  ``///`` or ``//!``), of Markdown files and of Rust files (whose doc comments are Markdown), whose target or whose
  own file moves; a link whose target did not exist before is left to the next rule only;
- paths written from the repository root (``docs/prd/krummi/pager/pager.prd.md``) of the moved files, in any text file and
  wherever they are, link targets included, when no path character comes right before or after them (a full stop
  that ends a sentence is none).

It checks the whole move before writing anything: every file must be tracked, and every destination new and
different from the others. Then it moves the files with ``git mv``, so that their history follows them.
"""

import os
import posixpath
import re
import subprocess
import sys
from pathlib import Path

# An inline Markdown link: the target, then an optional title. Targets with spaces or angle brackets are not used here.
INLINE = re.compile(r'(\]\()([^)\s<>]+)((?:\s+"[^"]*")?\))')
# A reference definition of a Markdown file: at the start of a line, at most three spaces in (four make a code block);
# a label that starts with `^` is a footnote (`[^1]: text`), not a link.
MD_REFERENCE = re.compile(r'^( {0,3}\[(?!\^)[^\]]+\]:[ \t]*)(\S+)', re.MULTILINE)
# One of a Rust doc comment, after its `///` or `//!`. A line of prose may start like one ("[`Store::open`]: when it
# fails"), and rustdoc's own targets (`crate::x`) are no files, so only a target with a `/` or a `.` counts.
RS_REFERENCE = re.compile(r'^([ \t]*//[/!][ \t]?[ \t]{0,3}\[(?!\^)[^\]]+\]:[ \t]*)(\S*[/.]\S*)', re.MULTILINE)
# Files whose links are Markdown: documents, and Rust's doc comments.
MARKDOWN = ('.md', '.rs')
# A target with a scheme (http:, mailto:) or a pure anchor is not a path.
NOT_A_PATH = re.compile(r'^(?:[a-zA-Z][a-zA-Z0-9+.-]*:|#)')
# What may come right before and after a path from the root for it to be that path and not part of a longer one: no
# path character before it, and after it no path character but a full stop that ends a sentence.
BEFORE = r'(?<![\w./-])'
AFTER = r'(?![\w/-]|\.\w)'
# The files of an execution: its document, closing report, phase log and measurements.
SUFFIXES = ('.md', '-report.md', '-phases.tsv', '-measurements.md')


def git(root, *args):
    return subprocess.run(['git', *args], cwd=root, check=True, capture_output=True, text=True).stdout


def repo_root():
    return Path(git(Path.cwd(), 'rev-parse', '--show-toplevel').strip())


def tracked(root):
    return [line for line in git(root, 'ls-files', '-z').split('\0') if line]


def read_text(root, path):
    """The file's text, or None for a file that is not UTF-8 text."""
    try:
        data = (root / path).read_bytes()
    except OSError:
        return None
    if b'\0' in data:
        return None
    try:
        return data.decode('utf-8')
    except UnicodeDecodeError:
        return None


def split_target(target):
    """The path of a link target and what follows it (an anchor or a query), apart."""
    cut = min((i for i in (target.find('#'), target.find('?')) if i >= 0), default=len(target))
    return target[:cut], target[cut:]


def reference_pattern(source):
    """The pattern of the reference definitions of `source`, by its kind."""
    return RS_REFERENCE if source.endswith('.rs') else MD_REFERENCE


def link_path(source, target):
    """The path from the root that a link target of `source` names, with the target's path part and what follows it;
    None for a target that names no file of the repository (a scheme, an anchor alone, an absolute path)."""
    if NOT_A_PATH.match(target) or target.startswith('/'):
        return None
    path, rest = split_target(target)
    if not path:
        return None
    return posixpath.normpath(posixpath.join(posixpath.dirname(source), path)), path, rest


def exists(root, files, dirs, path):
    return path in files or path in dirs or (root / path).exists()


def rewrite_link(source, new_source, target, moves, files, dirs, root):
    """The target of a link of `source` (moving to `new_source`), rewritten for the moves; unchanged when neither the
    link's file nor its target moves, or when the target did not exist."""
    named = link_path(source, target)
    if named is None:
        return target
    old, path, rest = named
    if old not in moves and source == new_source:
        return target
    if not exists(root, files, dirs, old):
        return target
    new = moves.get(old, old)
    rel = posixpath.relpath(new, posixpath.dirname(new_source) or '.')
    if path.endswith('/') and not rel.endswith('/'):
        rel += '/'
    return rel + rest


def rewrite_text(source, text, moves, files, dirs, root):
    """`text` of `source` with its references to the moves rewritten."""
    new_source = moves.get(source, source)
    if source.endswith(MARKDOWN):
        def inline(m):
            return m[1] + rewrite_link(source, new_source, m[2], moves, files, dirs, root) + m[3]

        def reference(m):
            return m[1] + rewrite_link(source, new_source, m[2], moves, files, dirs, root)

        text = INLINE.sub(inline, text)
        text = reference_pattern(source).sub(reference, text)
    for old in sorted(moves, key=len, reverse=True):
        if old in text:
            text = re.sub(BEFORE + re.escape(old) + AFTER, lambda _, new=moves[old]: new, text)
    return text


def directories(files):
    out = set()
    for f in files:
        parts = f.split('/')[:-1]
        for i in range(1, len(parts) + 1):
            out.add('/'.join(parts[:i]))
    return out


def move(root, moves):
    """Moves `moves` (old path to new path, from the root) and rewrites the references to them; returns the number of
    files whose text changed. Nothing is written when the move is not valid."""
    files = tracked(root)
    known = set(files)
    seen = set()
    for old, new in moves.items():
        if old not in known:
            raise SystemExit(f'not a tracked file: {old}')
        if new in known or (root / new).exists():
            raise SystemExit(f'the destination already exists: {new}')
        if new in seen:
            raise SystemExit(f'two files move to {new}')
        seen.add(new)
    dirs = directories(files)
    changed = 0
    for f in files:
        text = read_text(root, f)
        if text is None:
            continue
        new_text = rewrite_text(f, text, moves, known, dirs, root)
        if new_text != text:
            (root / f).write_bytes(new_text.encode('utf-8'))
            changed += 1
    for old, new in moves.items():
        (root / new).parent.mkdir(parents=True, exist_ok=True)
        git(root, 'mv', old, new)
    return changed


def archive_moves(root, feature, module):
    moves = {}
    for suffix in SUFFIXES:
        old = f'odd/tasks/{feature}{suffix}'
        if (root / old).exists():
            moves[old] = f'odd/done/{module}/{feature}{suffix}'
    if f'odd/tasks/{feature}.md' not in moves:
        raise SystemExit(f'no execution document odd/tasks/{feature}.md')
    return moves


def reopen_moves(root, feature):
    found = sorted((root / 'odd/done').glob(f'*/{feature}.md'))
    if len(found) != 1:
        raise SystemExit(f'no single archived execution {feature}: {[str(f) for f in found]}')
    module = found[0].parent.name
    moves = {}
    for suffix in SUFFIXES:
        old = f'odd/done/{module}/{feature}{suffix}'
        if (root / old).exists():
            moves[old] = f'odd/tasks/{feature}{suffix}'
    return moves


def manifest_moves(path):
    moves = {}
    for number, line in enumerate(Path(path).read_text(encoding='utf-8').splitlines(), 1):
        if not line.strip() or line.startswith('#'):
            continue
        parts = line.split('\t')
        if len(parts) != 2:
            raise SystemExit(f'{path}:{number}: expected an old path, a tab and a new path')
        moves[parts[0].strip()] = parts[1].strip()
    return moves


def broken_links(root):
    files = tracked(root)
    known = set(files)
    dirs = directories(files)
    out = []
    for f in files:
        if not f.endswith(MARKDOWN):
            continue
        text = read_text(root, f)
        if text is None:
            continue
        for number, line in enumerate(text.splitlines(), 1):
            targets = [m[2] for m in INLINE.finditer(line)]
            targets += [m[2] for m in reference_pattern(f).finditer(line)]
            for target in targets:
                named = link_path(f, target)
                if named is not None and not exists(root, known, dirs, named[0]):
                    out.append(f'{f}:{number}: {target}')
    return out


def main(argv):
    root = repo_root()
    here = Path.cwd()
    os.chdir(root)
    match argv:
        case ['archive', feature, module]:
            moves = archive_moves(root, feature, module)
        case ['reopen', feature]:
            moves = reopen_moves(root, feature)
        case ['move', manifest]:
            moves = manifest_moves(here / manifest)
        case ['check']:
            broken = broken_links(root)
            print('\n'.join(broken) if broken else 'no broken links')
            return 1 if broken else 0
        case _:
            print(__doc__)
            return 2
    changed = move(root, moves)
    print(f'moved {len(moves)} files, rewrote references in {changed} files')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
