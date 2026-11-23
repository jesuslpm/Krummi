"""Generate the Unicode tables of the text column collation (IC2 of the collation PRD, 7.7).

Reads the Unicode 17.0.0 data vendored in crates/krummi/unicode/17.0.0 and writes
crates/krummi/src/table/collation_tables.rs. Standard library only; the output is deterministic.

    python -I scripts/unicode/gen_collation_tables.py
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERSION = "17.0.0"
DATA = ROOT / "crates" / "krummi" / "unicode" / VERSION
OUTPUT = ROOT / "crates" / "krummi" / "src" / "table" / "collation_tables.rs"

S_BASE, L_BASE, V_BASE, T_BASE = 0xAC00, 0x1100, 0x1161, 0x11A7
L_COUNT, V_COUNT, T_COUNT = 19, 21, 28
N_COUNT = V_COUNT * T_COUNT
S_COUNT = L_COUNT * N_COUNT

# Bits of the quick check: the scalar may change the text, or may interact with its neighbours, under the collation.
QUICK_CI = 1
QUICK_CI_AI = 2


def lines(name: str):
    """The data lines of a UCD file, without comments, split on ';' and trimmed."""
    with open(DATA / name, encoding="utf-8") as f:
        for line in f:
            line = line.split("#", 1)[0].strip()
            if line:
                yield [field.strip() for field in line.split(";")]


def code_range(field: str) -> range:
    lo, _, hi = field.partition("..")
    return range(int(lo, 16), int(hi or lo, 16) + 1)


class Ucd:
    def __init__(self) -> None:
        self.ccc: dict[int, int] = {}
        self.decomposition: dict[int, list[int]] = {}
        self.mn: set[int] = set()
        for f in lines("UnicodeData.txt"):
            code = int(f[0], 16)
            if f[1].endswith(", Last>") or f[1].endswith(", First>"):
                # Ranges of ideographs, Hangul syllables, private use and the like: no class, no decomposition, no Mn.
                assert f[2] != "Mn" and f[3] == "0" and not f[5], f
                continue
            if f[3] != "0":
                self.ccc[code] = int(f[3])
            if f[2] == "Mn":
                self.mn.add(code)
            if f[5] and not f[5].startswith("<"):
                self.decomposition[code] = [int(x, 16) for x in f[5].split()]

        excluded = {c for f in lines("CompositionExclusions.txt") for c in code_range(f[0])}
        self.composition: dict[tuple[int, int], int] = {}
        for code, d in self.decomposition.items():
            # Primary composites: canonical pairs without the full composition exclusions (the listed exclusions,
            # singletons and non-starter decompositions).
            if len(d) == 2 and code not in excluded and self.class_of(code) == 0 and self.class_of(d[0]) == 0:
                self.composition[(d[0], d[1])] = code
        self.seconds = {b for (_, b) in self.composition}
        self.seconds.update(range(V_BASE, V_BASE + V_COUNT))
        self.seconds.update(range(T_BASE + 1, T_BASE + T_COUNT))

        self.folding = {int(f[0], 16): [int(x, 16) for x in f[2].split()] for f in lines("CaseFolding.txt") if f[1] in ("C", "F")}

        self.lgc: set[int] = set()
        for f in lines("Scripts.txt"):
            if f[1] in ("Latin", "Greek", "Cyrillic"):
                self.lgc.update(code_range(f[0]))

    def class_of(self, c: int) -> int:
        return self.ccc.get(c, 0)

    def full_decomposition(self, c: int) -> list[int]:
        if c in self.decomposition:
            return [x for d in self.decomposition[c] for x in self.full_decomposition(d)]
        return [c]

    # The folding, as IC2 defines it, for the quick check.

    def decompose(self, c: int, out: list[int]) -> None:
        if S_BASE <= c < S_BASE + S_COUNT:
            i = c - S_BASE
            out.append(L_BASE + i // N_COUNT)
            out.append(V_BASE + (i % N_COUNT) // T_COUNT)
            if i % T_COUNT:
                out.append(T_BASE + i % T_COUNT)
            return
        for x in self.full_decomposition(c):
            out.append(x)
            k = len(out) - 1
            while k > 0 and self.class_of(out[k]) != 0 and self.class_of(out[k - 1]) > self.class_of(out[k]):
                out[k - 1], out[k] = out[k], out[k - 1]
                k -= 1

    def nfd(self, text: list[int]) -> list[int]:
        out: list[int] = []
        for c in text:
            self.decompose(c, out)
        return out

    def compose_pair(self, a: int, b: int) -> int | None:
        if L_BASE <= a < L_BASE + L_COUNT and V_BASE <= b < V_BASE + V_COUNT:
            return S_BASE + ((a - L_BASE) * V_COUNT + (b - V_BASE)) * T_COUNT
        if S_BASE <= a < S_BASE + S_COUNT and (a - S_BASE) % T_COUNT == 0 and T_BASE < b < T_BASE + T_COUNT:
            return a + (b - T_BASE)
        return self.composition.get((a, b))

    def nfc(self, text: list[int]) -> list[int]:
        out: list[int] = []
        starter = None
        last = 0
        for c in self.nfd(text):
            cc = self.class_of(c)
            if starter is not None and (starter + 1 == len(out) or last < cc):
                p = self.compose_pair(out[starter], c)
                if p is not None:
                    out[starter] = p
                    continue
            if cc == 0:
                starter = len(out)
            out.append(c)
            last = cc
        return out

    def ci(self, text: list[int]) -> list[int]:
        folded = [x for c in self.nfd(text) for x in self.folding.get(c, [c])]
        return self.nfc(folded)

    def ci_ai(self, text: list[int]) -> list[int]:
        out: list[int] = []
        base = None
        prev = None
        for c in self.nfd(self.ci(text)):
            before, prev = prev, c
            if c in self.mn:
                if base is not None and base in self.lgc and not (c == 0x303 and before == ord("n")):
                    continue
            else:
                base = c
            out.append(c)
        return self.nfc(out)

    def quick_check(self) -> dict[int, int]:
        """The scalars that may change a text or interact with their neighbours, with their QUICK_* bits.

        A scalar is stable under `ci` when it is a starter, is not the second of a composition pair, its decomposition
        does not start with one, and it folds to itself alone; a text of stable scalars folds to itself. Under `ci_ai`
        its decomposition must also not start with an Mn, so the base of every mark it holds is its own (as in `ñ`),
        and it must fold to itself alone under `ci_ai` too."""
        candidates = (
            set(self.decomposition) | set(self.folding) | set(self.ccc) | self.seconds | self.mn
        )
        flags: dict[int, int] = {}
        for c in sorted(candidates):
            d = self.nfd([c])
            unstable_ci = self.class_of(c) != 0 or c in self.seconds or d[0] in self.seconds or self.ci([c]) != [c]
            unstable_ai = unstable_ci or d[0] in self.mn or self.ci_ai([c]) != [c]
            bits = (QUICK_CI if unstable_ci else 0) | (QUICK_CI_AI if unstable_ai else 0)
            if bits:
                flags[c] = bits
        return flags


def ranges(values: dict[int, int]) -> list[tuple[int, int, int]]:
    """Runs of consecutive scalars with the same value."""
    out: list[tuple[int, int, int]] = []
    for c in sorted(values):
        v = values[c]
        if out and out[-1][1] == c - 1 and out[-1][2] == v:
            out[-1] = (out[-1][0], c, v)
        else:
            out.append((c, c, v))
    return out


def lit(c: int) -> str:
    return f"'\\u{{{c:X}}}'"


def chars(cs: list[int]) -> str:
    return "&[" + ", ".join(lit(c) for c in cs) + "]"


def table(name: str, ty: str, doc: str, entries: list[str]) -> str:
    """A static slice, several entries to a line."""
    body: list[str] = []
    line = "   "
    for e in entries:
        if len(line) + 1 + len(e) + 1 > 120:
            body.append(line)
            line = "   "
        line += " " + e + ","
    if line.strip():
        body.append(line)
    return f"/// {doc}\n#[rustfmt::skip]\npub(super) static {name}: {ty} = &[\n" + "\n".join(body) + "\n];\n"


def render(ucd: Ucd) -> str:
    decompositions = {c: ucd.full_decomposition(c) for c in ucd.decomposition}
    lgc_ranges = ranges({c: 1 for c in ucd.lgc if not 0xD800 <= c <= 0xDFFF})
    parts = [
        f"// @generated by scripts/unicode/gen_collation_tables.py from the Unicode {VERSION} data in\n"
        f"// crates/krummi/unicode/{VERSION}. Do not edit: run `python -I scripts/unicode/gen_collation_tables.py`.\n"
        "\n"
        f"//! The Unicode {VERSION} tables of the collation's folding (IC2 of the collation PRD, 7.7). Every table is\n"
        "//! sorted by scalar value and searched by binary search. Hangul syllables decompose and compose by the\n"
        "//! algorithm of Unicode 3.12, not through these tables.\n",
        table(
            "DECOMPOSITION",
            "&[(char, &[char])]",
            "Full canonical decompositions (UnicodeData.txt field 5 without a tag, expanded recursively).",
            [f"({lit(c)}, {chars(d)})" for c, d in sorted(decompositions.items())],
        ),
        table(
            "COMBINING_CLASS",
            "&[(char, char, u8)]",
            "Inclusive ranges of the scalars with the same non-zero canonical combining class.",
            [f"({lit(lo)}, {lit(hi)}, {v})" for lo, hi, v in ranges(ucd.ccc)],
        ),
        table(
            "COMPOSITION",
            "&[(char, char, char)]",
            "Primary composites by their pair (first, second, composite), sorted by the pair.",
            [f"({lit(a)}, {lit(b)}, {lit(c)})" for (a, b), c in sorted(ucd.composition.items())],
        ),
        table(
            "CASE_FOLDING",
            "&[(char, &[char])]",
            "Full case folding: the CaseFolding.txt entries of status C and F.",
            [f"({lit(c)}, {chars(f)})" for c, f in sorted(ucd.folding.items())],
        ),
        table(
            "NONSPACING_MARK",
            "&[(char, char)]",
            "Inclusive ranges of the scalars of General_Category Mn.",
            [f"({lit(lo)}, {lit(hi)})" for lo, hi, _ in ranges({c: 1 for c in ucd.mn})],
        ),
        table(
            "LATIN_GREEK_CYRILLIC",
            "&[(char, char)]",
            "Inclusive ranges of the scalars of Script Latin, Greek or Cyrillic.",
            [f"({lit(lo)}, {lit(hi)})" for lo, hi, _ in lgc_ranges],
        ),
        f"/// Quick check bit: the scalar may change a text, or interact with its neighbours, under `ci`.\n"
        f"pub(super) const QUICK_CI: u8 = {QUICK_CI};\n"
        f"/// Quick check bit: the scalar may change a text, or interact with its neighbours, under `ci_ai`.\n"
        f"pub(super) const QUICK_CI_AI: u8 = {QUICK_CI_AI};\n",
        table(
            "QUICK_CHECK",
            "&[(char, char, u8)]",
            "Inclusive ranges of the scalars with the same non-zero quick check bits; a text whose scalars all lack a\n"
            "/// collation's bit is its own folded form under that collation.",
            [f"({lit(lo)}, {lit(hi)}, {v})" for lo, hi, v in ranges(ucd.quick_check())],
        ),
    ]
    return "\n".join(parts)


def main() -> int:
    text = render(Ucd())
    with open(OUTPUT, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print(f"{OUTPUT.relative_to(ROOT).as_posix()}: {len(text.encode('utf-8'))} bytes")
    return 0


if __name__ == "__main__":
    sys.exit(main())
