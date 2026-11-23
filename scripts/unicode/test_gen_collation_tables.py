"""Tests of gen_collation_tables.py: the committed tables are the generator's output, and a few known entries.

    python -I -m unittest discover -s scripts/unicode
"""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gen_collation_tables as gen  # noqa: E402


class GeneratorTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ucd = gen.Ucd()

    def test_committed_tables_are_up_to_date(self):
        committed = gen.OUTPUT.read_text(encoding="utf-8")
        self.assertEqual(committed, gen.render(self.ucd), "run python -I scripts/unicode/gen_collation_tables.py")

    def test_output_is_deterministic(self):
        self.assertEqual(gen.render(self.ucd), gen.render(gen.Ucd()))

    def test_known_entries(self):
        u = self.ucd
        self.assertEqual(u.full_decomposition(0x1EC7), [0x65, 0x323, 0x302])  # ệ
        self.assertEqual(u.composition[(0x6E, 0x303)], 0xF1)  # n + tilde = ñ
        self.assertNotIn((0x915, 0x93C), u.composition)  # क़ is a composition exclusion
        self.assertEqual(u.folding[0xDF], [0x73, 0x73])  # ß, status F
        self.assertNotIn(0x49, [c for c, f in u.folding.items() if f == [0x131]])  # no Turkic (T) entry
        self.assertEqual(u.ci_ai([ord(c) for c in "Año"]), [ord(c) for c in "año"])
        self.assertEqual(u.ci_ai([0x6E, 0x301, 0x303]), [0x6E])


if __name__ == "__main__":
    unittest.main()
