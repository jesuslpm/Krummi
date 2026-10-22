"""Tests for prd_time.py. Run: python -m unittest discover -s .claude/skills/prd/scripts"""

import contextlib
import io
import json
import os
import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path
from unittest import mock

import prd_time

T0 = datetime(2026, 10, 15, 10, 0, 0, tzinfo=timezone.utc)
SESSION = "bd359d3a-70a8-430d-813a-307fe28bf9dc"


def at(minutes, seconds=0):
    return (T0 + timedelta(minutes=minutes, seconds=seconds)).isoformat().replace("+00:00", "Z")


def human(minutes, text="hola", seconds=0):
    return {"type": "user", "timestamp": at(minutes, seconds), "origin": {"kind": "human"},
            "message": {"role": "user", "content": text}}


def assistant(minutes, seconds=0, tool=None):
    content = [{"type": "text", "text": "ok"}]
    if tool:
        content = [{"type": "tool_use", "id": tool[0], "name": tool[1], "input": {}}]
    return {"type": "assistant", "timestamp": at(minutes, seconds),
            "message": {"role": "assistant", "content": content}}


def tool_result(minutes, tool_id, seconds=0, permission=None):
    row = {"type": "user", "timestamp": at(minutes, seconds),
           "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": tool_id}]}}
    if permission:
        row["permissionDecision"] = {"decision": "accept", "source": permission}
    return row


def write_jsonl(path, rows):
    with open(path, "w", encoding="utf-8") as handle:
        for row in rows:
            handle.write(json.dumps(row) + "\n")


class MeasureTests(unittest.TestCase):
    def measure(self, rows, idle_minutes=10):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "t.jsonl"
            write_jsonl(path, rows)
            return prd_time.measure(prd_time.load_events(path), timedelta(minutes=idle_minutes))

    def test_agent_work_counts_in_full(self):
        result = self.measure([human(0), assistant(1), assistant(3)])
        self.assertEqual(result.wall, timedelta(minutes=3))
        self.assertEqual(result.agent, timedelta(minutes=3))
        self.assertEqual(result.human, timedelta(0))
        self.assertEqual(result.discarded, timedelta(0))
        self.assertEqual(result.interventions, 0)
        self.assertEqual(result.start, T0)

    def test_short_human_wait_counts_in_full(self):
        result = self.measure([human(0), assistant(2), human(9), assistant(10)])
        self.assertEqual(result.human, timedelta(minutes=7))
        self.assertEqual(result.agent, timedelta(minutes=3))
        self.assertEqual(result.discarded, timedelta(0))
        self.assertEqual(result.interventions, 1)

    def test_long_human_wait_counts_only_up_to_the_threshold(self):
        result = self.measure([human(0), assistant(2), human(32), assistant(33)])
        self.assertEqual(result.wall, timedelta(minutes=33))
        self.assertEqual(result.human, timedelta(minutes=10))
        self.assertEqual(result.discarded, timedelta(minutes=20))
        self.assertEqual(result.agent, timedelta(minutes=3))

    def test_threshold_is_configurable(self):
        result = self.measure([human(0), assistant(2), human(32)], idle_minutes=5)
        self.assertEqual(result.human, timedelta(minutes=5))
        self.assertEqual(result.discarded, timedelta(minutes=25))

    def test_answer_to_a_question_tool_is_a_human_wait(self):
        rows = [human(0), assistant(1, tool=("q1", "AskUserQuestion")), tool_result(31, "q1"), assistant(32)]
        result = self.measure(rows)
        self.assertEqual(result.human, timedelta(minutes=10))
        self.assertEqual(result.discarded, timedelta(minutes=20))
        self.assertEqual(result.interventions, 1)

    def test_user_permission_approval_is_a_human_wait(self):
        rows = [human(0), assistant(1, tool=("b1", "Bash")), tool_result(16, "b1", permission="user_temporary")]
        result = self.measure(rows)
        self.assertEqual(result.human, timedelta(minutes=10))
        self.assertEqual(result.discarded, timedelta(minutes=5))

    def test_long_tool_run_is_agent_work_and_never_discarded(self):
        rows = [human(0), assistant(1, tool=("a1", "Agent")), tool_result(26, "a1", permission="config"),
                assistant(27)]
        result = self.measure(rows)
        self.assertEqual(result.agent, timedelta(minutes=27))
        self.assertEqual(result.discarded, timedelta(0))

    def test_task_notification_is_not_a_human_message(self):
        notification = {"type": "user", "timestamp": at(20), "origin": {"kind": "task-notification"},
                        "message": {"role": "user", "content": "<task-notification>done</task-notification>"}}
        result = self.measure([human(0), assistant(1), notification, assistant(21)])
        self.assertEqual(result.agent, timedelta(minutes=21))
        self.assertEqual(result.interventions, 0)

    def test_rows_before_the_first_prompt_side_chains_and_meta_rows_are_ignored(self):
        early = {"type": "attachment", "timestamp": at(-5)}
        early_user = {"type": "user", "timestamp": at(-3), "isMeta": True,
                      "message": {"role": "user", "content": "hook output"}}
        side = assistant(50)
        side["isSidechain"] = True
        meta = {"type": "user", "timestamp": at(4), "isMeta": True,
                "message": {"role": "user", "content": [{"type": "text", "text": "skill body"}]}}
        result = self.measure([early, early_user, human(0), assistant(2), meta, assistant(5), side])
        self.assertEqual(result.start, T0)
        self.assertEqual(result.wall, timedelta(minutes=5))
        self.assertEqual(result.agent, timedelta(minutes=5))

    def test_rows_out_of_order_are_sorted_by_time(self):
        result = self.measure([human(0), assistant(3), assistant(1)])
        self.assertEqual(result.wall, timedelta(minutes=3))
        self.assertEqual(result.agent, timedelta(minutes=3))

    def test_a_transcript_without_a_human_prompt_is_an_error(self):
        with self.assertRaises(prd_time.PrdTimeError):
            self.measure([assistant(0), assistant(1)])


def record(session=SESSION, minutes=10):
    return prd_time.SessionRecord(session=session, start=T0.isoformat(), wall=minutes * 60, discarded=0,
                                  agent=minutes * 60, human=0, interventions=2, idle_minutes=10)


class SectionTests(unittest.TestCase):
    def test_section_is_appended_at_the_end_in_spanish(self):
        text = "# PRD\n\n## Referencias\n\n- algo\n"
        updated = prd_time.update_prd(text, record(), "es")
        self.assertTrue(updated.startswith(text))
        self.assertIn("## Tiempo de elaboración", updated)
        self.assertIn("| Sesión | Inicio | Reloj | Descartado | Activo | Agente | Persona | Intervenciones |", updated)
        self.assertIn("| bd359d3a |", updated)
        self.assertIn("10 min", updated)

    def test_section_labels_in_english(self):
        updated = prd_time.update_prd("# PRD\n", record(), "en")
        self.assertIn("## Elaboration time", updated)
        self.assertIn("| Session | Start | Wall clock | Discarded | Active | Agent | Person | Interventions |", updated)

    def test_rerunning_the_same_session_replaces_its_row(self):
        once = prd_time.update_prd("# PRD\n", record(minutes=10), "es")
        twice = prd_time.update_prd(once, record(minutes=25), "es")
        self.assertEqual(twice.count("| bd359d3a |"), 1)
        self.assertIn("25 min", twice)
        self.assertEqual(twice.count("## Tiempo de elaboración"), 1)

    def test_a_second_session_adds_a_row_and_a_total(self):
        once = prd_time.update_prd("# PRD\n", record(minutes=10), "es")
        twice = prd_time.update_prd(once, record(session="0123abcd-0000", minutes=65), "es")
        self.assertIn("| bd359d3a |", twice)
        self.assertIn("| 0123abcd |", twice)
        self.assertIn("| **Total** |", twice)
        self.assertIn("1 h 15 min", twice)

    def test_content_after_the_section_is_preserved(self):
        once = prd_time.update_prd("# PRD\n", record(), "es")
        with_tail = once + "\n## Anexo\n\nTexto.\n"
        again = prd_time.update_prd(with_tail, record(minutes=20), "es")
        self.assertTrue(again.endswith("## Anexo\n\nTexto.\n"))
        self.assertIn("20 min", again)

    def test_template_placeholder_is_replaced(self):
        text = "# PRD\n\n## Referencias\n\n- algo\n\n## Tiempo de elaboración\n\n<Do not write this section.>\n"
        updated = prd_time.update_prd(text, record(), "es")
        self.assertNotIn("Do not write", updated)
        self.assertEqual(updated.count("## Tiempo de elaboración"), 1)
        self.assertIn("## Referencias\n\n- algo\n\n## Tiempo de elaboración", updated)

    def test_durations_round_to_minutes(self):
        self.assertEqual(prd_time.format_duration(29), "0 min")
        self.assertEqual(prd_time.format_duration(90), "2 min")
        self.assertEqual(prd_time.format_duration(3600 + 12 * 60), "1 h 12 min")
        self.assertEqual(prd_time.format_duration(7200), "2 h 0 min")


class CliTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.config = self.root / "claude"
        transcript_dir = self.config / "projects" / "C--repo"
        transcript_dir.mkdir(parents=True)
        write_jsonl(transcript_dir / f"{SESSION}.jsonl", [human(0), assistant(2), human(5), assistant(6)])
        self.prd = self.root / "cambio.prd.md"
        self.prd.write_bytes(b"# PRD\r\n\r\nTexto.\r\n")
        self.prd_en = self.root / "cambio.prd.en.md"
        self.prd_en.write_text("# PRD\n", encoding="utf-8")
        self.env = {"CLAUDE_CONFIG_DIR": str(self.config), "CLAUDE_CODE_SESSION_ID": SESSION}

    def tearDown(self):
        self.tmp.cleanup()

    def run_main(self, *args, env=None):
        out, err = io.StringIO(), io.StringIO()
        with mock.patch.dict(os.environ, env if env is not None else self.env, clear=False):
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                code = prd_time.main(list(args))
        return code, out.getvalue(), err.getvalue()

    def test_updates_every_prd_in_its_own_language_and_keeps_line_endings(self):
        code, out, _ = self.run_main(str(self.prd), str(self.prd_en))
        self.assertEqual(code, 0)
        spanish = self.prd.read_bytes().decode("utf-8")
        self.assertIn("## Tiempo de elaboración", spanish)
        self.assertNotIn("\n", spanish.replace("\r\n", ""))
        self.assertIn("## Elaboration time", self.prd_en.read_text(encoding="utf-8"))
        self.assertIn("6 min", out)

    def test_dry_run_prints_without_writing(self):
        before = self.prd.read_bytes()
        code, out, _ = self.run_main(str(self.prd), "--dry-run")
        self.assertEqual(code, 0)
        self.assertEqual(self.prd.read_bytes(), before)
        self.assertIn("## Tiempo de elaboración", out)

    def test_explicit_session_overrides_the_environment(self):
        env = dict(self.env, CLAUDE_CODE_SESSION_ID="not-this-one")
        code, _, _ = self.run_main(str(self.prd), "--session", SESSION, env=env)
        self.assertEqual(code, 0)

    def test_missing_transcript_exits_2_without_writing(self):
        before = self.prd.read_bytes()
        code, _, err = self.run_main(str(self.prd), "--session", "missing")
        self.assertEqual(code, 2)
        self.assertIn("missing", err)
        self.assertEqual(self.prd.read_bytes(), before)

    def test_no_session_at_all_exits_2(self):
        env = dict(self.env, CLAUDE_CODE_SESSION_ID="")
        code, _, err = self.run_main(str(self.prd), env=env)
        self.assertEqual(code, 2)
        self.assertIn("--session", err)

    def test_missing_prd_exits_2(self):
        code, _, err = self.run_main(str(self.root / "nope.prd.md"))
        self.assertEqual(code, 2)
        self.assertIn("nope.prd.md", err)


if __name__ == "__main__":
    unittest.main()
