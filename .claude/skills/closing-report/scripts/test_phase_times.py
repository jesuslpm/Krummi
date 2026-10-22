"""Tests for phase_times. Run: python -m unittest discover -s .claude/skills/closing-report/scripts"""

import io
import os
import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from unittest import mock

import phase_times as pt

T0 = datetime(2026, 10, 16, 7, 0, 0, tzinfo=timezone(timedelta(hours=1)))


def at(minutes, seconds=0):
    return T0 + timedelta(minutes=minutes, seconds=seconds)


def line(minutes, event, phase="", label="", seconds=0):
    return f"{at(minutes, seconds).isoformat()}\t{event}\t{phase}\t{label}\n"


class LogTests(unittest.TestCase):
    def setUp(self):
        fd, self.path = tempfile.mkstemp(suffix=".tsv")
        os.close(fd)
        os.remove(self.path)

    def tearDown(self):
        if os.path.exists(self.path):
            os.remove(self.path)

    def run_main(self, *argv, now=None):
        with mock.patch.object(pt, "now", return_value=now or T0), \
                mock.patch("sys.stdout", new_callable=io.StringIO) as out, \
                mock.patch("sys.stderr", new_callable=io.StringIO) as err:
            code = pt.main([*argv])
        return code, out.getvalue(), err.getvalue()

    def read(self):
        with open(self.path, encoding="utf-8") as f:
            return f.read()

    def test_mark_appends_a_tab_separated_line_with_the_current_time(self):
        code, _, _ = self.run_main("mark", self.path, "explore", "--label", "map of the PRD")
        self.assertEqual(code, 0)
        self.assertEqual(self.read(), f"{T0.isoformat()}\tmark\texplore\tmap of the PRD\n")

    def test_mark_with_start_records_the_real_start(self):
        self.run_main("mark", self.path, "implement", "--start")
        self.assertEqual(self.read(), f"{T0.isoformat()}\tstart\timplement\t\n")

    def test_unknown_phase_is_a_usage_error_and_writes_nothing(self):
        with self.assertRaises(SystemExit) as ctx:
            self.run_main("mark", self.path, "coding")
        self.assertEqual(ctx.exception.code, 2)
        self.assertFalse(os.path.exists(self.path))

    def test_bg_end_without_its_start_fails_and_writes_nothing(self):
        self.run_main("bg-start", self.path, "acceptance", "generator")
        code, _, err = self.run_main("bg-end", self.path, "acceptance", "query")
        self.assertEqual(code, 2)
        self.assertIn("acceptance/query", err)
        self.assertEqual(self.read().count("\n"), 1)

    def test_bg_start_of_a_task_already_open_fails_and_writes_nothing(self):
        self.run_main("bg-start", self.path, "acceptance", "generator")
        code, _, err = self.run_main("bg-start", self.path, "acceptance", "generator", now=at(5))
        self.assertEqual(code, 2)
        self.assertIn("acceptance/generator", err)
        self.assertEqual(self.read().count("\n"), 1)

    def test_report_of_a_time_without_offset_fails_without_traceback(self):
        with open(self.path, "w", encoding="utf-8", newline="\n") as f:
            f.write(line(0, "start", "implement") + "2026-10-16T07:10:00\tend\t\t\n")
        code, _, err = self.run_main("report", self.path)
        self.assertEqual(code, 2)
        self.assertIn("line 2", err)

    def test_tabs_and_newlines_in_labels_are_flattened(self):
        self.run_main("bg-start", self.path, "acceptance", "run\tA\nreference")
        self.assertEqual(self.read(), f"{T0.isoformat()}\tbg-start\tacceptance\trun A reference\n")

    def test_report_of_a_missing_log_fails(self):
        code, _, err = self.run_main("report", self.path)
        self.assertEqual(code, 2)
        self.assertIn(self.path, err)

    def test_report_prints_the_phase_table(self):
        with open(self.path, "w", encoding="utf-8", newline="\n") as f:
            f.write(line(0, "mark", "explore") + line(10, "start", "implement") + line(40, "end"))
        code, out, _ = self.run_main("report", self.path, "--lang", "en")
        self.assertEqual(code, 0)
        self.assertIn("| Code and tests | 30 min 00 s | 75 % | 1 |", out)


class SummaryTests(unittest.TestCase):
    def summarize(self, text):
        return pt.summarize(pt.parse_log(text))

    def test_foreground_phases_add_up_and_waits_stay_out_of_active_time(self):
        s = self.summarize(
            line(0, "mark", "explore")
            + line(12, "mark", "wait", "questions and estimate")
            + line(20, "start", "implement")
            + line(50, "mark", "checks")
            + line(55, "mark", "review")
            + line(65, "mark", "fix")
            + line(70, "mark", "explore")
            + line(73, "mark", "close")
            + line(80, "end")
        )
        self.assertEqual(s.foreground["explore"].seconds, 15 * 60)
        self.assertEqual(s.foreground["explore"].spans, 2)
        self.assertEqual(s.foreground["implement"].seconds, 30 * 60)
        self.assertNotIn("wait", s.foreground)
        self.assertEqual(s.waits, [(at(12), at(20), "questions and estimate")])
        self.assertEqual(s.active, timedelta(minutes=72))
        self.assertEqual(s.start, at(20))
        self.assertEqual(s.active_before_start, timedelta(minutes=12))
        self.assertIsNone(s.open_phase)

    def test_background_tasks_are_listed_apart_and_not_added_to_active_time(self):
        s = self.summarize(
            line(0, "start", "implement")
            + line(10, "bg-start", "acceptance", "generator")
            + line(10, "mark", "review")
            + line(16, "bg-end", "acceptance", "generator", seconds=30)
            + line(16, "bg-start", "acceptance", "query", seconds=30)
            + line(20, "end")
        )
        self.assertEqual(s.active, timedelta(minutes=20))
        self.assertEqual(s.background, [("acceptance", "generator", at(10), at(16, 30))])
        self.assertEqual(s.open_background, [("acceptance", "query", at(16, 30))])

    def test_a_log_without_end_reports_the_open_phase_without_counting_it(self):
        s = self.summarize(line(0, "start", "implement") + line(5, "mark", "acceptance"))
        self.assertEqual(s.active, timedelta(minutes=5))
        self.assertEqual(s.open_phase, ("acceptance", at(5)))

    def test_without_a_start_mark_everything_counts_after_the_start(self):
        s = self.summarize(line(0, "mark", "explore") + line(5, "end"))
        self.assertIsNone(s.start)
        self.assertEqual(s.active_before_start, timedelta(0))

    def test_a_mark_after_end_resumes_the_run(self):
        s = self.summarize(
            line(0, "start", "implement") + line(10, "end") + line(30, "mark", "fix") + line(35, "end")
        )
        self.assertEqual(s.active, timedelta(minutes=15))
        self.assertEqual(s.paused, [(at(10), at(30))])

    def test_a_time_without_utc_offset_is_rejected(self):
        with self.assertRaises(ValueError) as ctx:
            pt.parse_log("2026-10-16T07:00:00\tstart\timplement\t\n")
        self.assertIn("line 1", str(ctx.exception))
        self.assertIn("UTC offset", str(ctx.exception))

    def test_a_malformed_line_names_its_number(self):
        with self.assertRaises(ValueError) as ctx:
            pt.parse_log(line(0, "start", "implement") + "garbage\n")
        self.assertIn("line 2", str(ctx.exception))


COMMIT_LOG = (
    line(0, "start", "implement", "T1")
    + line(20, "mark", "checks")
    + line(25, "mark", "wait", "question")
    + line(30, "mark", "review")
    + line(40, "mark", "implement", "T2")
    + line(55, "mark", "close")
    + line(60, "end")
)
COMMITS = [("c1", at(22), "feat: a"), ("c2", at(50), "feat: b | c")]


class CommitTests(unittest.TestCase):
    def test_each_commit_gets_the_active_time_since_the_previous_commit(self):
        rows = pt.by_commit(pt.summarize(pt.parse_log(COMMIT_LOG)), COMMITS)
        self.assertEqual(
            [(r.sha, r.labels, r.phases) for r in rows],
            [
                ("c1", ["T1"], {"implement": 1200, "checks": 120}),
                ("c2", ["T2"], {"implement": 600, "checks": 180, "review": 600}),
                (None, ["T2"], {"implement": 300, "close": 300}),
            ],
        )

    def test_rows_add_up_to_the_active_time(self):
        s = pt.summarize(pt.parse_log(COMMIT_LOG))
        rows = pt.by_commit(s, COMMITS)
        self.assertEqual(sum(r.seconds for r in rows), int(s.active.total_seconds()))

    def test_commits_outside_the_log_get_no_time(self):
        rows = pt.by_commit(
            pt.summarize(pt.parse_log(COMMIT_LOG)),
            [("prd", at(-30), "docs(prd): x"), ("c2", at(50), "feat: b"), ("rep", at(90), "docs: report")],
        )
        self.assertEqual([(r.sha, r.seconds) for r in rows], [("prd", 0), ("c2", 2700), ("rep", 600), (None, 0)])

    def test_spanish_commit_table(self):
        rows = pt.by_commit(pt.summarize(pt.parse_log(COMMIT_LOG)), COMMITS)
        self.assertEqual(
            pt.render_commits(rows, "es").splitlines(),
            [
                "Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):",
                "",
                "| Commit | Mensaje | Etiquetas | Activo | Desglose |",
                "|---|---|---|---|---|",
                "| `c1` | feat: a | T1 | 22 min 00 s | Código y tests 20 min 00 s, "
                "Checks (fmt, clippy, suite de tests) 2 min 00 s |",
                "| `c2` | feat: b \\| c | T2 | 23 min 00 s | Código y tests 10 min 00 s, "
                "Checks (fmt, clippy, suite de tests) 3 min 00 s, Revisión RDD 10 min 00 s |",
                "| — | después del último commit | T2 | 10 min 00 s | Código y tests 5 min 00 s, "
                "Cierre (informe y merge) 5 min 00 s |",
                "| **Total** | | | **55 min 00 s** | |",
            ],
        )

    def test_report_with_commits_reads_git_log(self):
        out = "c1\t" + at(22).isoformat() + "\tfeat: a\n" + "c2\t" + at(50).isoformat() + "\tfeat: b | c\n"
        done = mock.Mock(returncode=0, stdout=out, stderr="")
        fd, path = tempfile.mkstemp(suffix=".tsv")
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as f:
            f.write(COMMIT_LOG)
        try:
            with mock.patch.object(pt.subprocess, "run", return_value=done) as run, \
                    mock.patch("sys.stdout", new_callable=io.StringIO) as stdout:
                code = pt.main(["report", path, "--commits", "abc..HEAD", "--lang", "en"])
        finally:
            os.remove(path)
        self.assertEqual(code, 0)
        argv = run.call_args.args[0]
        self.assertEqual(argv[-2:], ["--end-of-options", "abc..HEAD"])
        self.assertIn("| `c2` | feat: b \\| c | T2 | 23 min 00 s |", stdout.getvalue())
        self.assertIn("| — | after the last commit | T2 | 10 min 00 s |", stdout.getvalue())

    def test_git_failure_returns_2_without_traceback(self):
        failed = mock.Mock(returncode=128, stdout="", stderr="fatal: bad revision 'nope'")
        fd, path = tempfile.mkstemp(suffix=".tsv")
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as f:
            f.write(COMMIT_LOG)
        try:
            with mock.patch.object(pt.subprocess, "run", return_value=failed), \
                    mock.patch("sys.stdout", new_callable=io.StringIO), \
                    mock.patch("sys.stderr", new_callable=io.StringIO) as err:
                code = pt.main(["report", path, "--commits", "nope..HEAD"])
        finally:
            os.remove(path)
        self.assertEqual(code, 2)
        self.assertIn("fatal: bad revision", err.getvalue())


class RenderTests(unittest.TestCase):
    def test_spanish_report(self):
        s = pt.summarize(pt.parse_log(
            line(0, "mark", "explore")
            + line(12, "mark", "wait", "preguntas y estimación")
            + line(20, "start", "implement")
            + line(50, "bg-start", "acceptance", "generador, extracto")
            + line(50, "mark", "review")
            + line(65, "bg-end", "acceptance", "generador, extracto", seconds=20)
            + line(65, "mark", "close", seconds=20)
            + line(80, "end")
        ))
        self.assertEqual(
            pt.render(s, "es").splitlines(),
            [
                "| Fase | Duración | % del activo | Tramos |",
                "|---|---|---|---|",
                "| Exploración y mapa | 12 min 00 s | 17 % | 1 |",
                "| Código y tests | 30 min 00 s | 42 % | 1 |",
                "| Revisión RDD | 15 min 20 s | 21 % | 1 |",
                "| Cierre (informe y merge) | 14 min 40 s | 20 % | 1 |",
                "| **Total activo** | **1 h 12 min** | **100 %** | **4** |",
                "",
                "- Inicio: 2026-10-16 07:00:00 +01:00. Fin: 2026-10-16 08:20:00 +01:00.",
                "- Arranque confirmado: 07:20:00. Activo desde el arranque: **1 h 00 min** "
                "(compárese con la estimación); antes del arranque: 12 min 00 s.",
                "- Esperas por el usuario (fuera del activo): 8 min 00 s.",
                "  - 07:12:00–07:20:00 (8 min 00 s): preguntas y estimación",
                "",
                "En segundo plano (se solapa con las fases de arriba; no suma al activo):",
                "",
                "| Fase | Tarea | Inicio | Fin | Duración |",
                "|---|---|---|---|---|",
                "| Aceptación | generador, extracto | 07:50:00 | 08:05:20 | 15 min 20 s |",
                "| **Aceptación (total)** | | | | **15 min 20 s** |",
            ],
        )

    def test_english_labels_and_open_items(self):
        s = pt.summarize(pt.parse_log(
            line(0, "start", "implement") + line(5, "bg-start", "acceptance", "run A") + line(5, "mark", "checks")
        ))
        text = pt.render(s, "en")
        self.assertIn("| Code and tests | 5 min 00 s | 100 % | 1 |", text)
        self.assertIn("Open phase, not counted: Checks (fmt, clippy, test suite) since 07:05:00.", text)
        self.assertIn("Background task still open, not counted: Acceptance / run A since 07:05:00.", text)

    def test_durations(self):
        self.assertEqual(pt.duration(timedelta(seconds=45)), "0 min 45 s")
        self.assertEqual(pt.duration(timedelta(minutes=59, seconds=59)), "59 min 59 s")
        self.assertEqual(pt.duration(timedelta(hours=1, minutes=3, seconds=40)), "1 h 03 min")


if __name__ == "__main__":
    unittest.main()
