"""Tests for numstat_by_area. Run: python -m unittest discover -s .claude/skills/closing-report/scripts"""

import io
import os
import subprocess
import tempfile
import unittest
from unittest import mock

import numstat_by_area as nba

B, C, K = nba.BLANK, nba.CODE, nba.COMMENT


class AreaTests(unittest.TestCase):
    def test_crate_files_group_by_crate(self):
        self.assertEqual(nba.area_of("crates/krummi-core/src/main.rs"), "crates/krummi-core")

    def test_other_folders_group_by_first_segment(self):
        self.assertEqual(nba.area_of("docs/methodology/slides.md"), "docs")

    def test_root_files_are_their_own_area(self):
        self.assertEqual(nba.area_of("README.md"), "README.md")


class ParseTests(unittest.TestCase):
    def test_plain_entry(self):
        self.assertEqual(nba.parse_numstat_z("3\t1\tdocs/a.md\0"), [("3", "1", "docs/a.md")])

    def test_rename_keeps_the_full_destination_path(self):
        # With -z git writes renames as an empty path field followed by old and new paths.
        data = "5\t2\t\0crates/x/src/old.rs\0crates/x/src/new.rs\0"
        self.assertEqual(nba.parse_numstat_z(data), [("5", "2", "crates/x/src/new.rs")])

    def test_non_ascii_path_is_not_quoted(self):
        data = "1\t0\tdocs/methodology/generación.md\0"
        self.assertEqual(nba.parse_numstat_z(data), [("1", "0", "docs/methodology/generación.md")])

    def test_several_entries(self):
        data = "1\t0\ta.md\0" "-\t-\timg.png\0" "2\t2\t\0x/old.md\0x/new.md\0"
        self.assertEqual(
            nba.parse_numstat_z(data),
            [("1", "0", "a.md"), ("-", "-", "img.png"), ("2", "2", "x/new.md")],
        )


class RustClassifierTests(unittest.TestCase):
    def kinds(self, text):
        return nba.classify("src/lib.rs", text)

    def test_line_doc_and_inner_doc_comments(self):
        text = "// note\n/// doc\n//! crate doc\nfn f() {} // trailing\n\n    \n"
        self.assertEqual(self.kinds(text), [K, K, K, C, B, B, B])

    def test_nested_block_comment_ends_where_the_outer_one_ends(self):
        text = "/* a\n /* nested */\n still comment\n*/\nlet x = 1; /* c */\n/* c */ let y = 2;"
        self.assertEqual(self.kinds(text), [K, K, K, K, C, C])

    def test_comment_markers_inside_strings_are_code(self):
        text = 'let u = "http://x";\nlet s = "a\n// inside a string\n";\nlet r = r#"/* "# ;\nlet q = \'"\';'
        self.assertEqual(self.kinds(text), [C, C, C, C, C, C])

    def test_raw_string_has_no_escapes_and_lifetimes_do_not_open_strings(self):
        text = "let p = r\"C:\\\";\n// after\nfn f<'a>(x: &'a str) {}\n// after lifetime\nlet c = '\\'';\n// end"
        self.assertEqual(self.kinds(text), [C, K, C, K, C, K])

    def test_byte_raw_strings_and_identifiers_ending_in_r(self):
        text = 'let b = br"\\";\n// after\nlet n = for_r"x";\n// after identifier'
        self.assertEqual(self.kinds(text), [C, K, C, K])

    def test_attributes_are_code(self):
        self.assertEqual(self.kinds("#[derive(Debug)]\n#![allow(dead_code)]"), [C, C])


class PythonClassifierTests(unittest.TestCase):
    def kinds(self, text):
        return nba.classify("scripts/x.py", text)

    def test_docstrings_and_hash_comments_are_comments(self):
        text = (
            '"""Module doc.\n\nMore.\n"""\n'
            "# comment\n"
            "def f():\n"
            "    '''Function doc.'''\n"
            "    return 1  # trailing\n"
        )
        self.assertEqual(self.kinds(text), [K, B, K, K, K, C, K, C, B])

    def test_a_string_that_is_not_a_docstring_is_code(self):
        text = 'x = 1\n"""not a docstring"""\ns = """a\n# not a comment\n"""\n'
        self.assertEqual(self.kinds(text), [C, C, C, C, C, B])

    def test_syntax_error_falls_back_to_hash_comments(self):
        self.assertEqual(self.kinds("def (:\n# comment\n"), [C, K, B])


class HashClassifierTests(unittest.TestCase):
    def test_powershell_block_comments_and_here_strings(self):
        text = (
            "<#\n.SYNOPSIS\n  Help.\n#>\n"
            "$a = 1 # trailing\n"
            "# comment\n"
            '$h = @"\n# not a comment\n"@\n'
            "<# one line #> $b = 2\n"
            "$s = 'a # b'"
        )
        kinds = nba.classify("scripts/x.ps1", text)
        self.assertEqual(kinds, [K, K, K, K, C, K, C, C, C, C, C])

    def test_toml_comments_and_hash_inside_strings(self):
        text = '# comment\nname = "a#b"  # trailing\n\n[deps]'
        self.assertEqual(nba.classify("Cargo.toml", text), [K, C, B, C])

    def test_javascript_strings_and_comments(self):
        text = "// c\nconst u = 'http://x';\nconst t = `\n// in template\n`;\n/* c */"
        self.assertEqual(nba.classify("site/app.mjs", text), [K, C, C, C, C, K])

    def test_javascript_apostrophe_does_not_open_a_string_past_its_line(self):
        text = "const t = <p>Don't</p>;\n// still a comment\nconst u = 1;"
        self.assertEqual(nba.classify("site/app.jsx", text), [C, K, C])

    def test_toml_multi_line_strings_hide_hash_lines(self):
        text = 'doc = """\n# not a comment\n"""\n# comment\ns = \'\'\'one line\'\'\' # trailing'
        self.assertEqual(nba.classify("Cargo.toml", text), [C, C, C, K, C])

    def test_files_that_are_not_code_have_no_classifier(self):
        self.assertIsNone(nba.classifier_for("docs/a.md"))
        self.assertIsNone(nba.classifier_for("data.json"))
        self.assertIsNotNone(nba.classifier_for("crates/a/src/lib.rs"))


class AddedLinesTests(unittest.TestCase):
    def test_hunks_give_the_added_line_numbers_of_each_destination(self):
        patch = (
            "diff --git crates/a/src/lib.rs crates/a/src/lib.rs\n"
            "--- crates/a/src/lib.rs\n"
            "+++ crates/a/src/lib.rs\n"
            "@@ -1,0 +2,2 @@\n"
            "+++ a content line that looks like a header\n"
            "+// second\n"
            "@@ -9 +11 @@\n"
            "-old\n"
            "+new\n"
            "\\ No newline at end of file\n"
            "diff --git old.rs gone.rs\n"
            "--- gone.rs\n"
            "+++ /dev/null\n"
            "@@ -1 +0,0 @@\n"
            "-x\n"
            '+++ "docs/tab\\there.rs"\t\n'
            "@@ -0,0 +1 @@\n"
            "+y\n"
        )
        self.assertEqual(
            nba.added_lines(patch),
            {"crates/a/src/lib.rs": [2, 3, 11], "docs/tab\there.rs": [1]},
        )

    def test_quoted_paths_decode_octal_utf8(self):
        self.assertEqual(nba.unquote_path('"docs/generaci\\303\\263n.rs"'), "docs/generación.rs")
        self.assertEqual(nba.unquote_path("plain path.rs"), "plain path.rs")

    def test_breakdown_counts_code_and_comments_of_the_added_lines_only(self):
        kinds = [K, C, B, C, K]
        self.assertEqual(nba.count_added(kinds, [1, 2, 3, 5]), (1, 2))


class SummaryTests(unittest.TestCase):
    def test_lockfiles_binaries_and_apart_globs_stay_out_of_totals(self):
        entries = [
            ("10", "2", "crates/a/src/lib.rs"),
            ("4", "0", "Cargo.lock"),
            ("-", "-", "docs/logo.png"),
            ("900", "0", "docs/methodology/slides.html"),
            ("3", "1", "docs/methodology/slides.md"),
        ]
        areas, apart = nba.summarize(entries, ["docs/methodology/slides.html"])
        self.assertEqual(areas["crates/a"], nba.Counts(added=10, removed=2, files=1, code_files=1))
        self.assertEqual(areas["docs"], nba.Counts(added=3, removed=1, files=1))
        self.assertEqual(
            [path for path, _, _ in apart],
            ["Cargo.lock", "docs/logo.png", "docs/methodology/slides.html"],
        )

    def test_breakdown_adds_up_by_area_for_code_files_only(self):
        entries = [("10", "2", "crates/a/src/lib.rs"), ("5", "0", "crates/a/src/x.rs"), ("3", "0", "crates/a/README.md")]
        breakdown = {"crates/a/src/lib.rs": (6, 3), "crates/a/src/x.rs": (4, 1)}
        areas, _ = nba.summarize(entries, [], breakdown)
        self.assertEqual(
            areas["crates/a"],
            nba.Counts(added=18, removed=2, files=3, code=10, comments=4, code_files=2),
        )


LEGEND_ES = (
    "Código + y Comentarios + desglosan las líneas añadidas en archivos de código (Rust, Python, PowerShell, "
    "JavaScript/TypeScript, TOML, YAML y shell); las líneas en blanco no cuentan en ninguna de las dos."
)


class RenderTests(unittest.TestCase):
    def test_table_is_sorted_by_change_size_with_spanish_grouping_and_apart_line(self):
        areas = {
            "docs": nba.Counts(10, 0, 1),
            "crates/a": nba.Counts(1500, 20, 3, code=1100, comments=310, code_files=3),
        }
        text = nba.render(areas, [("Cargo.lock", "4", "0")], "es")
        self.assertEqual(
            text.splitlines(),
            [
                "| Área | + | − | Código + | Comentarios + | Archivos |",
                "|---|---|---|---|---|---|",
                "| `crates/a` | 1.500 | 20 | 1.100 | 310 | 3 |",
                "| `docs` | 10 | 0 | — | — | 1 |",
                "| **Total** | **1.510** | **20** | **1.100** | **310** | **4** |",
                "",
                LEGEND_ES,
                "",
                "Aparte (lockfiles, generados o binarios): `Cargo.lock` (+4 −0)",
            ],
        )

    def test_english_labels_and_grouping(self):
        text = nba.render({"docs": nba.Counts(1500, 0, 1)}, [], "en")
        self.assertIn("| Area | + | − | Code + | Comments + | Files |", text)
        self.assertIn("| **Total** | **1,500** | **0** | **—** | **—** | **1** |", text)
        self.assertIn("Code + and Comments + split the lines added", text)


def git(cwd, *args):
    subprocess.run(
        ["git", "-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", *args],
        cwd=cwd, check=True, capture_output=True,
    )


class EndToEndTests(unittest.TestCase):
    def test_real_repository_split_of_a_rust_and_a_markdown_change(self):
        with tempfile.TemporaryDirectory() as repo:
            git(repo, "init", "-q")
            os.makedirs(os.path.join(repo, "crates", "a", "src"))
            lib = os.path.join(repo, "crates", "a", "src", "lib.rs")
            with open(lib, "w", encoding="utf-8", newline="\n") as f:
                f.write("/// Doc.\nfn f() {}\n")
            git(repo, "add", ".")
            git(repo, "commit", "-q", "--no-verify", "-m", "base")
            with open(lib, "w", encoding="utf-8", newline="\n") as f:
                f.write("/// Doc.\nfn f() {}\n\n// New comment.\n/* block\n   more */\nfn g() {} // trailing\n")
            with open(os.path.join(repo, "README.md"), "w", encoding="utf-8", newline="\n") as f:
                f.write("# Title\n\nText.\n")
            git(repo, "add", ".")
            git(repo, "commit", "-q", "--no-verify", "-m", "head")
            cwd = os.getcwd()
            os.chdir(repo)
            out = io.TextIOWrapper(io.BytesIO(), encoding="utf-8")  # main() reconfigures stdout
            try:
                with mock.patch("sys.stdout", out):
                    self.assertEqual(nba.main(["HEAD~1", "--lang", "en"]), 0)
            finally:
                os.chdir(cwd)
        out.flush()
        lines = out.buffer.getvalue().decode("utf-8").splitlines()
        self.assertIn("| `crates/a` | 5 | 0 | 1 | 3 | 1 |", lines)
        self.assertIn("| `README.md` | 3 | 0 | — | — | 1 |", lines)
        self.assertIn("| **Total** | **8** | **0** | **1** | **3** | **2** |", lines)


class MainTests(unittest.TestCase):
    def test_git_failure_returns_2_without_traceback(self):
        failed = mock.Mock(returncode=128, stdout="", stderr="fatal: bad revision")
        with mock.patch.object(nba.subprocess, "run", return_value=failed) as run, \
                mock.patch("sys.stderr", new_callable=io.StringIO) as err:
            self.assertEqual(nba.main(["nope"]), 2)
        self.assertIn("fatal: bad revision", err.getvalue())
        self.assertIn("--end-of-options", run.call_args.args[0])

    def test_unreadable_code_file_loses_only_its_split(self):
        numstat = mock.Mock(returncode=0, stdout="3\t0\tcrates/a/src/lib.rs\0", stderr="")
        patch = mock.Mock(returncode=0, stdout=b"+++ crates/a/src/lib.rs\n@@ -0,0 +1,3 @@\n+a\n+b\n+c\n", stderr=b"")
        show = mock.Mock(returncode=128, stdout=b"", stderr=b"fatal: bad object")
        out = io.TextIOWrapper(io.BytesIO(), encoding="utf-8")
        with mock.patch.object(nba.subprocess, "run", side_effect=[numstat, patch, show]), \
                mock.patch("sys.stdout", out), mock.patch("sys.stderr", new_callable=io.StringIO) as err:
            self.assertEqual(nba.main(["base"]), 0)
        out.flush()
        self.assertIn("| `crates/a` | 3 | 0 | — | — | 1 |", out.buffer.getvalue().decode("utf-8"))
        self.assertIn("warning: no code/comment split for crates/a/src/lib.rs: fatal: bad object", err.getvalue())

    def test_apart_globs_match_case_exactly(self):
        _, apart = nba.summarize([("5", "0", "docs/Slides.HTML")], ["*.html"])
        self.assertEqual(apart, [])


class ArgsTests(unittest.TestCase):
    def test_defaults(self):
        args = nba.parse_args(["abc123"])
        self.assertEqual((args.base, args.head, args.lang, args.apart), ("abc123", "HEAD", "es", []))

    def test_a_ref_named_like_the_language_is_kept(self):
        args = nba.parse_args(["es", "en", "--lang", "en"])
        self.assertEqual((args.base, args.head, args.lang), ("es", "en", "en"))

    def test_lang_without_value_is_a_usage_error(self):
        with self.assertRaises(SystemExit) as ctx:
            nba.parse_args(["abc123", "--lang"])
        self.assertEqual(ctx.exception.code, 2)

    def test_unknown_lang_is_a_usage_error(self):
        with self.assertRaises(SystemExit) as ctx:
            nba.parse_args(["abc123", "--lang", "fr"])
        self.assertEqual(ctx.exception.code, 2)


if __name__ == "__main__":
    unittest.main()
