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
        with self.assertRaises(SystemExit):
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
        for moves in ({'a.md': 'b.md'}, {'c.md': 'd.md'}):
            with self.assertRaises(SystemExit):
                move_docs.move(repo.root, moves)
        self.assertEqual(repo.read('a.md'), '[b](b.md)\n')
        self.assertEqual(repo.tracked(), {'a.md', 'b.md'})


    def test_a_relative_manifest_is_read_from_the_current_folder(self):
        repo = self.repo({'docs/a.md': 'a\n', 'tools/m.tsv': 'docs/a.md\tdocs/b/a.md\n'})
        here = os.getcwd()
        self.addCleanup(os.chdir, here)
        os.chdir(repo.root / 'tools')
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(move_docs.main(['move', 'm.tsv']), 0)
        self.assertEqual(repo.tracked(), {'docs/b/a.md', 'tools/m.tsv'})

    def test_a_malformed_manifest_line_is_named(self):
        repo = self.repo({'a.md': 'a\n'})
        manifest = repo.root / 'm.tsv'
        manifest.write_text('# moves\na.md\tb.md\na.md b.md\n', encoding='utf-8')
        with self.assertRaisesRegex(SystemExit, r'm\.tsv:3: expected an old path, a tab and a new path'):
            move_docs.manifest_moves(manifest)

    def test_two_files_moving_to_one_place_change_nothing(self):
        repo = self.repo({'a.md': '[b](b.md)\n', 'b.md': '[a](a.md)\n'})
        with self.assertRaisesRegex(SystemExit, 'two files move to c.md'):
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


if __name__ == '__main__':
    unittest.main()
