"""Tests of move_docs.py, each on a fresh git repository in a temporary folder.

Run with `python -m unittest discover -s scripts/docs`.
"""

import contextlib
import io
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import move_docs  # noqa: E402


class Repo:
    """A temporary git repository with the given files committed."""

    def __init__(self, files):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        for path, text in files.items():
            (self.root / path).parent.mkdir(parents=True, exist_ok=True)
            (self.root / path).write_bytes(text.encode('utf-8'))
        for args in (['init', '-q'], ['add', '-A'],
                     ['-c', 'user.name=t', '-c', 'user.email=t@t', 'commit', '-q', '-m', 'start']):
            subprocess.run(['git', *args], cwd=self.root, check=True, capture_output=True)

    def read(self, path):
        return (self.root / path).read_text(encoding='utf-8')

    def tracked(self):
        return set(move_docs.tracked(self.root))

    def close(self):
        self.tmp.cleanup()


class MoveTests(unittest.TestCase):
    def repo(self, files):
        repo = Repo(files)
        self.addCleanup(repo.close)
        return repo

    def test_links_follow_moved_targets_and_moved_files(self):
        repo = self.repo({
            'docs/prd/a.prd.md': '# A\n\n[Child](b.prd.md#goal) and [review](../reviews/a/r.md).\n',
            'docs/prd/b.prd.md': '# B\n\nParent: [A](a.prd.md).\n',
            'docs/reviews/a/r.md': 'See [A](../../prd/a.prd.md#L3) and [kept](../../prd/b.prd.md).\n',
            'README.md': '[A](docs/prd/a.prd.md) [site](https://example.com/x.md) [top](#top)\n',
        })
        changed = move_docs.move(repo.root, {'docs/prd/a.prd.md': 'docs/prd/mod/a.prd.md'})
        self.assertEqual(changed, 4)
        self.assertEqual(repo.read('docs/prd/mod/a.prd.md'),
                         '# A\n\n[Child](../b.prd.md#goal) and [review](../../reviews/a/r.md).\n')
        self.assertEqual(repo.read('docs/prd/b.prd.md'), '# B\n\nParent: [A](mod/a.prd.md).\n')
        self.assertEqual(repo.read('docs/reviews/a/r.md'),
                         'See [A](../../prd/mod/a.prd.md#L3) and [kept](../../prd/b.prd.md).\n')
        self.assertEqual(repo.read('README.md'),
                         '[A](docs/prd/mod/a.prd.md) [site](https://example.com/x.md) [top](#top)\n')
        self.assertIn('docs/prd/mod/a.prd.md', repo.tracked())
        self.assertNotIn('docs/prd/a.prd.md', repo.tracked())

    def test_links_in_rust_doc_comments_follow_their_target(self):
        repo = self.repo({
            'docs/prd/a.prd.md': '# A\n',
            'crates/x/src/lib.rs': '//! Module 6 ([a.prd.md](../../../docs/prd/a.prd.md#k1)).\nfn f() { v[0](1) }\n',
        })
        move_docs.move(repo.root, {'docs/prd/a.prd.md': 'docs/prd/m/a.prd.md'})
        self.assertEqual(repo.read('crates/x/src/lib.rs'),
                         '//! Module 6 ([a.prd.md](../../../docs/prd/m/a.prd.md#k1)).\nfn f() { v[0](1) }\n')

    def test_files_moved_together_keep_their_links(self):
        repo = self.repo({
            'odd/tasks/f.md': '[report](f-report.md)\n',
            'odd/tasks/f-report.md': '[document](f.md)\n',
        })
        moves = move_docs.archive_moves(repo.root, 'f', 'trees')
        self.assertEqual(moves, {'odd/tasks/f.md': 'odd/done/trees/f.md',
                                 'odd/tasks/f-report.md': 'odd/done/trees/f-report.md'})
        move_docs.move(repo.root, moves)
        self.assertEqual(repo.read('odd/done/trees/f.md'), '[report](f-report.md)\n')
        self.assertEqual(repo.read('odd/done/trees/f-report.md'), '[document](f.md)\n')

    def test_archiving_moves_every_file_of_the_execution(self):
        repo = self.repo({
            'odd/tasks/f.md': 'x\n', 'odd/tasks/f-report.md': 'x\n', 'odd/tasks/f-phases.tsv': 'x\n',
            'odd/tasks/f-measurements.md': 'x\n', 'odd/tasks/fg.md': 'other\n',
        })
        move_docs.move(repo.root, move_docs.archive_moves(repo.root, 'f', 'pager'))
        self.assertEqual(repo.tracked(), {'odd/done/pager/f.md', 'odd/done/pager/f-report.md',
                                          'odd/done/pager/f-phases.tsv', 'odd/done/pager/f-measurements.md',
                                          'odd/tasks/fg.md'})
        with self.assertRaisesRegex(SystemExit, 'No existe el documento de ejecución odd/tasks/missing.md'):
            move_docs.archive_moves(repo.root, 'missing', 'pager')

    def test_reopening_brings_an_archived_execution_back(self):
        repo = self.repo({
            'odd/done/trees/f.md': '[report](f-report.md)\n', 'odd/done/trees/f-report.md': 'x\n',
            'docs/r.md': '[f](../odd/done/trees/f.md)\n',
        })
        move_docs.move(repo.root, move_docs.reopen_moves(repo.root, 'f'))
        self.assertEqual(repo.tracked(), {'odd/tasks/f.md', 'odd/tasks/f-report.md', 'docs/r.md'})
        self.assertEqual(repo.read('docs/r.md'), '[f](../odd/tasks/f.md)\n')
        with self.assertRaises(SystemExit):
            move_docs.reopen_moves(repo.root, 'f')

    def test_paths_from_the_root_are_rewritten_only_as_whole_paths(self):
        repo = self.repo({
            'odd/tasks/f.md': 'x\n',
            'src/lib.rs': ('// D1 of odd/tasks/f.md, L4: see odd/tasks/f.md.\n'
                           '// Not these: odd/tasks/f.md.bak, x/odd/tasks/f.md, odd/tasks/f.mdx, odd/tasks/f.md/a\n'),
            'notes.md': 'Logged in `odd/tasks/f.md` (L2) and [in a link](odd/tasks/f.md#L2).\n',
        })
        move_docs.move(repo.root, {'odd/tasks/f.md': 'odd/done/m/f.md'})
        self.assertEqual(repo.read('src/lib.rs'), (
            '// D1 of odd/done/m/f.md, L4: see odd/done/m/f.md.\n'
            '// Not these: odd/tasks/f.md.bak, x/odd/tasks/f.md, odd/tasks/f.mdx, odd/tasks/f.md/a\n'))
        self.assertEqual(repo.read('notes.md'),
                         'Logged in `odd/done/m/f.md` (L2) and [in a link](odd/done/m/f.md#L2).\n')

    def test_broken_links_stay_as_they_are_and_are_reported(self):
        repo = self.repo({
            'docs/a.md': '[gone](missing.md) [b](b.md)\n[ref]: ../nowhere.md\n',
            'docs/b.md': 'b\n',
        })
        self.assertEqual(move_docs.broken_links(repo.root), ['docs/a.md:1: missing.md', 'docs/a.md:2: ../nowhere.md'])
        move_docs.move(repo.root, {'docs/a.md': 'docs/sub/a.md'})
        self.assertEqual(repo.read('docs/sub/a.md'), '[gone](missing.md) [b](../b.md)\n[ref]: ../nowhere.md\n')
        self.assertEqual(move_docs.broken_links(repo.root),
                         ['docs/sub/a.md:1: missing.md', 'docs/sub/a.md:2: ../nowhere.md'])

    def test_a_move_onto_an_existing_file_or_of_an_untracked_one_changes_nothing(self):
        repo = self.repo({'a.md': '[b](b.md)\n', 'b.md': 'b\n'})
        for moves, message in (({'a.md': 'b.md'}, 'El destino ya existe: b.md'),
                               ({'c.md': 'd.md'}, 'No es un archivo bajo control de versiones: c.md')):
            with self.assertRaisesRegex(SystemExit, message):
                move_docs.move(repo.root, moves)
        self.assertEqual(repo.read('a.md'), '[b](b.md)\n')
        self.assertEqual(repo.tracked(), {'a.md', 'b.md'})

    def test_a_destination_inside_a_file_changes_nothing(self):
        # Codex R1: a folder of the destination that is a file, or another destination.
        repo = self.repo({'a.md': 'a\n', 'b.md': 'b\n', 'blocked': 'x\n', 'README.md': '[a](a.md) [b](b.md)\n'})
        for moves, message in (
                ({'a.md': 'blocked/a.md'}, 'El destino blocked/a.md queda dentro de blocked, que es un archivo'),
                ({'a.md': 'x.md', 'b.md': 'x.md/b.md'}, 'El destino x.md/b.md queda dentro de x.md, otro destino')):
            with self.assertRaisesRegex(SystemExit, message):
                move_docs.move(repo.root, moves)
        self.assertEqual(repo.read('README.md'), '[a](a.md) [b](b.md)\n')
        self.assertEqual(repo.tracked(), {'a.md', 'b.md', 'blocked', 'README.md'})

    def test_paths_of_a_move_are_compared_normalized(self):
        # Codex R1, round 2: `./x.md` is `x.md`, so it is the folder of the other destination; and no path may leave
        # the repository.
        repo = self.repo({'a.md': 'a\n', 'b.md': 'b\n', 'README.md': '[a](a.md) [b](b.md)\n'})
        for moves, message in (
                ({'a.md': './x.md', 'b.md': 'x.md/b.md'}, 'El destino x.md/b.md queda dentro de x.md, otro destino'),
                ({'./a.md': 'c.md', 'a.md': 'd.md'}, 'El archivo a.md aparece dos veces'),
                ({'a.md': '../a.md'}, 'La ruta ../a.md queda fuera del repositorio'),
                ({'a.md': '/a.md'}, 'La ruta /a.md queda fuera del repositorio'),
                ({'a.md': 'C:/a.md'}, 'La ruta C:/a.md queda fuera del repositorio'),
                # RDD R3-case-alias-ancestor: Windows and macOS take these for one path.
                ({'a.md': 'X.md', 'b.md': 'x.md/b.md'}, 'El destino x.md/b.md queda dentro de x.md, otro destino'),
                ({'a.md': 'C.md', 'b.md': 'c.md'}, 'Dos archivos se mueven a c.md')):
            with self.assertRaisesRegex(SystemExit, message):
                move_docs.move(repo.root, moves)
        self.assertEqual(repo.read('README.md'), '[a](a.md) [b](b.md)\n')
        self.assertEqual(repo.tracked(), {'a.md', 'b.md', 'README.md'})
        move_docs.move(repo.root, {'./a.md': 'docs/./a.md', 'b.md': 'docs\\b.md'})
        self.assertEqual(repo.read('README.md'), '[a](docs/a.md) [b](docs/b.md)\n')
        self.assertEqual(repo.tracked(), {'docs/a.md', 'docs/b.md', 'README.md'})

    def test_windows_spellings_are_refused(self):
        # Codex R1, round 3: Windows drops a final dot or space (`x.md.` is `x.md`) and reserves some names.
        repo = self.repo({'a.md': 'a\n', 'b.md': 'b\n', 'README.md': '[a](a.md) [b](b.md)\n'})
        for moves, message in (
                ({'a.md': 'x.md', 'b.md': 'x.md./b.md'}, 'La ruta x.md./b.md tiene un nombre que Windows no admite: x.md.'),
                ({'a.md': 'docs /a.md'}, 'La ruta docs /a.md tiene un nombre que Windows no admite: docs '),
                ({'a.md': 'nul.md'}, 'La ruta nul.md tiene un nombre que Windows no admite: nul.md'),
                ({'a.md': 'Com1/a.md'}, 'La ruta Com1/a.md tiene un nombre que Windows no admite: Com1'),
                ({'a.md': 'a?.md'}, r'La ruta a\?.md tiene un nombre que Windows no admite: a\?.md')):
            with self.assertRaisesRegex(SystemExit, message):
                move_docs.move(repo.root, moves)
        self.assertEqual(repo.read('README.md'), '[a](a.md) [b](b.md)\n')
        self.assertEqual(repo.tracked(), {'a.md', 'b.md', 'README.md'})

    def test_a_move_that_fails_midway_is_undone(self):
        # Codex R1, round 3: whatever alias the checks miss, a failed move leaves the repository as it was.
        repo = self.repo({'a.md': '[b](b.md)\n', 'b.md': 'b\n', 'README.md': '[a](a.md) [b](b.md)\n'})
        real_git, calls = move_docs.git, []

        def failing_git(root, *args):
            if args[:1] == ('mv',):
                calls.append(args)
                if len(calls) == 2:
                    raise subprocess.CalledProcessError(128, ['git', *args])
            return real_git(root, *args)

        move_docs.git = failing_git
        self.addCleanup(setattr, move_docs, 'git', real_git)
        with self.assertRaisesRegex(SystemExit, 'se deshicieron sus cambios'):
            move_docs.move(repo.root, {'a.md': 'docs/a.md', 'b.md': 'docs/b.md'})
        move_docs.git = real_git
        self.assertEqual((repo.read('a.md'), repo.read('README.md')), ('[b](b.md)\n', '[a](a.md) [b](b.md)\n'))
        self.assertEqual(repo.tracked(), {'a.md', 'b.md', 'README.md'})
        self.assertEqual(move_docs.git(repo.root, 'status', '--porcelain'), '')
        self.assertFalse((repo.root / 'docs').exists())

    def test_reference_destinations_keep_their_parentheses(self):
        # Codex R4: a reference definition's destination may hold parentheses, as it did before angle brackets.
        repo = self.repo({'guide(v2).md': 'g\n', 'odd/tasks/f.md': 'See [the guide][g].\n\n[g]: ../../guide(v2).md\n'})
        self.assertEqual(move_docs.broken_links(repo.root), [])
        move_docs.move(repo.root, move_docs.archive_moves(repo.root, 'f', 'tooling'))
        self.assertEqual(repo.read('odd/done/tooling/f.md'), 'See [the guide][g].\n\n[g]: ../../../guide(v2).md\n')
        self.assertEqual(move_docs.broken_links(repo.root), [])

    def test_angle_bracket_destinations_follow_their_target(self):
        # Codex R2: `<...>` destinations, with a space inside, inline and in reference definitions.
        repo = self.repo({
            'README.md': 'r\n', 'my notes.md': 'n\n',
            'odd/tasks/f.md': ('[readme](<../../README.md>) [notes](<../../my notes.md#top> "Notes")\n\n'
                               '[root]: <../../README.md>\n'),
            'src/lib.rs': '//! [f](<../odd/tasks/f.md>)\n//! [g]: <../odd/tasks/f.md>\n',
        })
        self.assertEqual(move_docs.broken_links(repo.root), [])
        move_docs.move(repo.root, move_docs.archive_moves(repo.root, 'f', 'tooling'))
        self.assertEqual(repo.read('odd/done/tooling/f.md'),
                         '[readme](<../../../README.md>) [notes](<../../../my notes.md#top> "Notes")\n\n'
                         '[root]: <../../../README.md>\n')
        self.assertEqual(repo.read('src/lib.rs'),
                         '//! [f](<../odd/done/tooling/f.md>)\n//! [g]: <../odd/done/tooling/f.md>\n')
        self.assertEqual(move_docs.broken_links(repo.root), [])

    def test_a_relative_manifest_is_read_from_the_current_folder(self):
        repo = self.repo({'docs/a.md': 'a\n', 'tools/m.tsv': 'docs/a.md\tdocs/b/a.md\n'})
        here = os.getcwd()
        self.addCleanup(os.chdir, here)
        os.chdir(repo.root / 'tools')
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(move_docs.main(['move', 'm.tsv']), 0)
        self.assertEqual(repo.tracked(), {'docs/b/a.md', 'tools/m.tsv'})
        # Codex R3: the messages of the command are in Spanish.
        self.assertEqual(out.getvalue(), 'Se movieron 1 archivos y se reescribieron referencias en 1 archivos.\n')
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(move_docs.main(['check']), 0)
        self.assertEqual(out.getvalue(), 'No hay enlaces rotos.\n')
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(move_docs.main(['otra']), 2)
        self.assertIn('Uso: python -I scripts/docs/move_docs.py', out.getvalue())

    def test_a_malformed_manifest_line_is_named(self):
        repo = self.repo({'a.md': 'a\n'})
        manifest = repo.root / 'm.tsv'
        manifest.write_text('# moves\na.md\tb.md\na.md b.md\n', encoding='utf-8')
        with self.assertRaisesRegex(SystemExit, r'm\.tsv:3: se esperaba una ruta antigua, un tabulador y una ruta nueva'):
            move_docs.manifest_moves(manifest)

    def test_two_files_moving_to_one_place_change_nothing(self):
        repo = self.repo({'a.md': '[b](b.md)\n', 'b.md': '[a](a.md)\n'})
        with self.assertRaisesRegex(SystemExit, 'Dos archivos se mueven a c.md'):
            move_docs.move(repo.root, {'a.md': 'c.md', 'b.md': 'c.md'})
        self.assertEqual((repo.read('a.md'), repo.read('b.md')), ('[b](b.md)\n', '[a](a.md)\n'))
        self.assertEqual(repo.tracked(), {'a.md', 'b.md'})

    def test_rust_reference_definitions_follow_their_target(self):
        repo = self.repo({
            'docs/a.md': 'a\n',
            'src/lib.rs': '//! See [the PRD].\n//!\n//! [the PRD]: ../docs/a.md#s1\n/// [doc]: ../docs/a.md\n',
        })
        move_docs.move(repo.root, {'docs/a.md': 'docs/m/a.md'})
        self.assertEqual(repo.read('src/lib.rs'),
                         '//! See [the PRD].\n//!\n//! [the PRD]: ../docs/m/a.md#s1\n/// [doc]: ../docs/m/a.md\n')

    def test_check_reports_broken_links_of_rust_doc_comments(self):
        repo = self.repo({
            'docs/a.md': 'a\n',
            # The last two doc lines start like reference definitions but name no file: prose, and a rustdoc path.
            'src/lib.rs': ('//! [ok](../docs/a.md) [gone](../docs/z.md)\n//! [ref]: ../nowhere.md\n'
                           '/// [`Store::open`]: when it fails, see a\n/// [x]: crate::y\nfn f() {}\n'),
        })
        self.assertEqual(move_docs.broken_links(repo.root),
                         ['src/lib.rs:1: ../docs/z.md', 'src/lib.rs:2: ../nowhere.md'])


    def test_markdown_reference_definitions_keep_their_rules(self):
        # A definition may name a file without an extension; one indented four spaces is a code block, not a link.
        repo = self.repo({
            'LICENSE': 'l\n',
            'docs/a.md': '[l]: ../LICENSE\n   [m]: ../LICENSE\n    [code]: ../LICENSE\n\n    [gone]: missing.md\n',
        })
        self.assertEqual(move_docs.broken_links(repo.root), [])
        move_docs.move(repo.root, {'LICENSE': 'legal/LICENSE'})
        self.assertEqual(repo.read('docs/a.md'), ('[l]: ../legal/LICENSE\n   [m]: ../legal/LICENSE\n'
                                                  '    [code]: ../LICENSE\n\n    [gone]: missing.md\n'))

    def test_footnotes_are_no_links(self):
        # Each footnote starts with a path, which a definition would take as its target: the first ones name a missing
        # file, which `check` would report; the second ones the file that moves, which `move` would rewrite.
        a = 'Text.[^1][^2]\n\n[^1]: ../zz.md is gone.\n[^2]: ../../outside/b.md\n'
        rs = '//! [^n]: ../zz.md is gone.\n//! [^m]: ../outside/b.md\n'
        repo = self.repo({'outside/b.md': 'b\n', 'docs/sub/a.md': a, 'src/lib.rs': rs})
        self.assertEqual(move_docs.broken_links(repo.root), [])
        move_docs.move(repo.root, {'outside/b.md': 'outside/m/b.md'})
        self.assertEqual((repo.read('docs/sub/a.md'), repo.read('src/lib.rs')), (a, rs))

    def test_reopening_refuses_an_execution_archived_twice(self):
        repo = self.repo({'odd/done/trees/f.md': 'x\n', 'odd/done/pager/f.md': 'y\n'})
        with self.assertRaisesRegex(SystemExit, 'No hay una única ejecución archivada f'):
            move_docs.reopen_moves(repo.root, 'f')


if __name__ == '__main__':
    unittest.main()
