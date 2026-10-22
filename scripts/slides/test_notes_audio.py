"""Tests for notes_audio.py: run with `python -m unittest discover -s scripts/slides`."""

import asyncio
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import notes_audio

DECK = """---
marp: true
---

<!-- _class: lead -->

# Primera slide

<!--
Cómo leerla:

Primera slide.

Texto que se
lee en voz alta.

▸ Mostrar: docs/prd/chicago-crime/.

⏱ Al terminar esta slide: hacia el minuto 1.
-->

---

## Segunda **slide**

- Un punto

<!--
Cómo leerla:

Segunda slide. Se dice con git.
-->
"""


class SpokenTest(unittest.TestCase):
    def test_presenter_lines_are_not_read(self):
        note = "\nPrimera.\n\n▸ Mostrar: odd/tasks/crime-query.md.\n\n⏱ Al terminar esta slide: hacia el minuto 3.\n"
        self.assertEqual(notes_audio.spoken(note), "Primera.")

    def test_wrapped_lines_join_and_paragraphs_stay_apart(self):
        note = "Una línea\npartida en dos.\n\nOtro párrafo."
        self.assertEqual(notes_audio.spoken(note), "Una línea partida en dos.\n\nOtro párrafo.")

    def test_a_wrapped_direction_is_not_read(self):
        note = "Texto.\n\n▸ Mostrar: una indicación larga\nque sigue en otra línea.\n\nMás texto."
        self.assertEqual(notes_audio.spoken(note), "Texto.\n\nMás texto.")

    def test_a_direction_inside_a_paragraph_is_not_read(self):
        note = "Texto que se lee.\n▸ Mostrar: algo\nque sigue.\n⏱ Al terminar esta slide: hacia el minuto 3.\n\nMás texto."
        self.assertEqual(notes_audio.spoken(note), "Texto que se lee.\n\nMás texto.")


class PronounceTest(unittest.TestCase):
    def test_table_words_are_spelled_as_spoken(self):
        cases = {
            "git worktree": "guit uórktri",
            "Git y los worktrees": "guit y los uórktris",
            "un commit, dos commits": "un cómit, dos cómits",
            "93 MiB de pico": "93 mebibytes de pico",
            "el skill y los skills": "el esquil y los esquils",
            "un hook, dos hooks": "un juk, dos juks",
            "una SSTable, dos SSTables": "una ese ese table, dos ese ese tables",
            "hace auditable el trabajo": "hace auditáble el trabajo",
            "CLAUDE.md y AGENTS.md": "Claude punto eme de y Agents punto eme de",
        }
        for written, said in cases.items():
            with self.subTest(written=written):
                self.assertEqual(notes_audio.pronounce(written), said)

    def test_words_that_only_contain_a_table_word_stay(self):
        text = "GitHub, gitignore, recommit y hookshot"
        self.assertEqual(notes_audio.pronounce(text), text)


class NotesTest(unittest.TestCase):
    def test_every_slide_gives_its_number_title_and_spoken_text(self):
        self.assertEqual(
            notes_audio.notes(DECK),
            [
                (1, "Primera slide", "Primera slide.\n\nTexto que se lee en voz alta."),
                (2, "Segunda slide", "Segunda slide. Se dice con git."),
            ],
        )

    def test_a_slide_without_notes_is_left_out(self):
        deck = DECK + "\n---\n\n## Sin notas\n"
        self.assertEqual([number for number, _, _ in notes_audio.notes(deck)], [1, 2])

    def test_the_real_deck_numbers_every_slide_as_marp_pages_it(self):
        # The presenter view plays audio/slide-NN.mp3 for Marp's page NN: every slide must have a note, so
        # notes() numbers 1..N without gaps, as Marp does.
        deck = (notes_audio.ROOT / "docs/methodology/slides.md").read_text(encoding="utf-8")
        numbers = [number for number, _, _ in notes_audio.notes(deck)]
        self.assertEqual(numbers, list(range(1, len(numbers) + 1)))
        self.assertGreater(len(numbers), 20)

    def test_a_byte_order_mark_does_not_shift_the_numbers(self):
        self.assertEqual(notes_audio.notes("\ufeff" + DECK), notes_audio.notes(DECK))


class GenerateTest(unittest.TestCase):
    ONLY_DIRECTIONS = "\n---\n\n## Solo indicaciones\n\n<!--\nCómo leerla:\n\n▸ Mostrar: algo.\n-->\n"

    SECOND = notes_audio.BITRATE // 8  # bytes in one second of audio

    def run_generate(self, deck: str, out: Path, fail: bool = False) -> list[str]:
        """Runs generate() with a stub voice that writes one second of audio made from the text (so moved files
        can be told apart), or fails when asked to; returns the texts it was given."""
        said = []

        async def one_second(text, voice, path):
            said.append(text)
            if fail:
                raise ConnectionError("sin conexión")
            path.write_bytes(text.encode().ljust(self.SECOND, b"\0")[: self.SECOND])

        slides = out / "slides.md"
        slides.write_text(deck, encoding="utf-8")
        with mock.patch.object(notes_audio, "synthesize", one_second), mock.patch("builtins.print") as printed:
            try:
                asyncio.run(notes_audio.generate(slides, out / "audio", "voz"))
            finally:
                self.printed = [" ".join(map(str, call.args)) for call in printed.call_args_list]
        return said

    def audio(self, out: Path) -> dict[str, bytes]:
        return {p.name: p.read_bytes() for p in sorted((out / "audio").glob("slide-*.mp3"))}

    def test_one_file_per_slide_with_notes_and_the_durations_table(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            (out / "audio").mkdir()
            (out / "audio" / "slide-09.mp3").write_bytes(b"old")
            said = self.run_generate(DECK, out)
            files = sorted(p.name for p in (out / "audio").iterdir())
            table = (out / "audio" / "duraciones.md").read_text(encoding="utf-8")
        self.assertEqual(said, ["Primera slide.\n\nTexto que se lee en voz alta.", "Segunda slide. Se dice con guit."])
        self.assertEqual(files, ["duraciones.md", "manifest.json", "slide-01.mp3", "slide-02.mp3"])
        self.assertIn("| 1 | Primera slide | 9 | 1 s | 0.0 min | 540 |", table)
        self.assertIn("| 2 | Segunda slide | 6 | 1 s | 0.0 min | 360 |", table)

    def test_unchanged_notes_are_not_synthesized_again(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            self.run_generate(DECK, out)
            before = self.audio(out)
            said = self.run_generate(DECK, out)
            after = self.audio(out)
        self.assertEqual(said, [])
        self.assertEqual(after, before)

    def test_every_slide_prints_its_duration_and_the_running_total(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            self.run_generate(DECK, out)
            self.run_generate(DECK, out)
        self.assertEqual(
            self.printed,
            [
                "slide 01    1.0 s  total   0.0 min  reutilizado  Primera slide",
                "slide 02    1.0 s  total   0.0 min  reutilizado  Segunda slide",
                "total 0.0 min",
            ],
        )

    def test_a_broken_manifest_means_a_full_regeneration(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            self.run_generate(DECK, out)
            (out / "audio" / "manifest.json").write_text("<<<<<<< HEAD\n[{\"file\": 1}]", encoding="utf-8")
            said = self.run_generate(DECK, out)
        self.assertEqual(len(said), 2)

    def test_only_a_changed_note_is_synthesized(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            self.run_generate(DECK, out)
            before = self.audio(out)
            said = self.run_generate(DECK.replace("Se dice con git.", "Se dice distinto."), out)
            after = self.audio(out)
        self.assertEqual(said, ["Segunda slide. Se dice distinto."])
        self.assertEqual(after["slide-01.mp3"], before["slide-01.mp3"])
        self.assertNotEqual(after["slide-02.mp3"], before["slide-02.mp3"])

    def test_an_inserted_slide_moves_the_next_files_without_synthesizing_them(self):
        inserted = DECK.replace("\n---\n\n## Segunda", "\n---\n\n## Nueva\n\n<!--\nCómo leerla:\n\nNueva.\n-->\n\n---\n\n## Segunda")
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            self.run_generate(DECK, out)
            before = self.audio(out)
            said = self.run_generate(inserted, out)
            after = self.audio(out)
        self.assertEqual(said, ["Nueva."])
        self.assertEqual(sorted(after), ["slide-01.mp3", "slide-02.mp3", "slide-03.mp3"])
        self.assertEqual(after["slide-03.mp3"], before["slide-02.mp3"])

    def test_a_failing_voice_leaves_the_previous_audio_untouched(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            self.run_generate(DECK, out)
            before = self.audio(out)
            manifest = (out / "audio" / "manifest.json").read_bytes()
            with self.assertRaises(ConnectionError):
                self.run_generate(DECK.replace("Se dice con git.", "Se dice distinto."), out, fail=True)
            self.assertEqual(self.audio(out), before)
            self.assertEqual((out / "audio" / "manifest.json").read_bytes(), manifest)

    def test_a_note_with_only_directions_is_skipped(self):
        with tempfile.TemporaryDirectory() as tmp:
            said = self.run_generate(DECK + self.ONLY_DIRECTIONS, Path(tmp))
            files = sorted(p.name for p in (Path(tmp) / "audio").iterdir())
        self.assertEqual(len(said), 2)
        self.assertNotIn("slide-03.mp3", files)


if __name__ == "__main__":
    unittest.main()
