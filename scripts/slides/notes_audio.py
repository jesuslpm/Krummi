# /// script
# requires-python = ">=3.10"
# dependencies = ["edge-tts==7.3.1"]
# ///
"""Turn the speaker notes of the Marp deck into one MP3 per slide, played from the presenter view.

Usage, from the repository root, with uv (https://docs.astral.sh/uv/):

    uv run scripts/slides/notes_audio.py [--slides docs/methodology/slides.md]
        [--out docs/methodology/audio] [--voice es-ES-AlvaroNeural]

uv reads the dependencies declared above and runs the script in an isolated environment of its own: nothing
is installed in the system Python. The versions of edge-tts and of everything it pulls in are locked in
notes_audio.py.lock; after changing the dependencies above, refresh it with
`uv lock --script scripts/slides/notes_audio.py`. The tests need only the standard library:
`uv run python -m unittest discover -s scripts/slides`.

Only the spoken text of each "Cómo leerla" note is read: a direction for the presenter ("▸") or a timing mark
("⏱") is skipped up to the next blank line. Words the voice gets wrong are respelled first (PRONUNCIATION); the
notes themselves keep the normal spelling. The output folder gets slide-NN.mp3 for every slide with notes,
duraciones.md with the length of each one and manifest.json with a fingerprint of each text. The folder is
committed, so a run only synthesizes the notes whose text changed: the others reuse their previous audio, moved
to the new slide number when slides are inserted or removed. Nothing is replaced until every slide has audio.
"""

import argparse
import asyncio
import hashlib
import json
import re
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BITRATE = 48_000  # edge-tts writes audio-24khz-48kbitrate-mono-mp3: constant bitrate, so size gives length
NOTE = re.compile(r"<!--\s*Cómo leerla:(.*?)-->", re.S)
SKIPPED = ("▸", "⏱")

# Written form -> how the voice must see it. Whole words only, so GitHub or gitignore stay as they are.
PRONUNCIATION = [
    (r"\bCLAUDE\.md\b", "Claude punto eme de"),
    (r"\bAGENTS\.md\b", "Agents punto eme de"),
    (r"(?i)\bgit\b", "guit"),
    (r"(?i)\bworktree(s?)\b", r"uórktri\1"),
    (r"(?i)\bcommit(s?)\b", r"cómit\1"),
    (r"\bMiB\b", "mebibytes"),
    (r"(?i)\bskill(s?)\b", r"esquil\1"),
    (r"(?i)\bhook(s?)\b", r"juk\1"),
    (r"\bSSTable(s?)\b", r"ese ese table\1"),
    (r"(?i)\bauditable(s?)\b", r"auditáble\1"),
]


def spoken(note: str) -> str:
    """The text of a note that is read aloud: wrapped lines joined, paragraphs kept apart. A line that starts
    with a SKIPPED mark begins a direction for the presenter: it and the lines after it, up to the next blank
    line, are left out, wherever the mark appears."""
    paragraphs, current, skipping = [], [], False
    for line in [*note.splitlines(), ""]:
        text = line.strip()
        if not text:  # a blank line ends both a spoken paragraph and a direction
            if current:
                paragraphs.append(" ".join(current))
            current, skipping = [], False
        elif skipping or text.startswith(SKIPPED):
            if current:
                paragraphs.append(" ".join(current))
                current = []
            skipping = True
        else:
            current.append(text)
    return "\n\n".join(paragraphs)


def pronounce(text: str) -> str:
    for pattern, said in PRONUNCIATION:
        text = re.sub(pattern, said, text)
    return text


def notes(markdown: str) -> list[tuple[int, str, str]]:
    """(slide number, title, spoken text) for every slide that has a "Cómo leerla" note."""
    markdown = markdown.lstrip("\ufeff")  # an editor's byte order mark would hide the front matter
    body = re.sub(r"\A---\n.*?\n---\n", "", markdown, count=1, flags=re.S)  # Marp front matter
    slides = [s for s in re.split(r"^---\s*$", body, flags=re.M) if s.strip()]
    found = []
    for number, slide in enumerate(slides, 1):
        note = NOTE.search(slide)
        if note:
            heading = re.search(r"^#{1,2}\s+(.+)$", slide, re.M)
            title = re.sub(r"[*_`]", "", heading.group(1)).strip() if heading else ""
            found.append((number, title, spoken(note.group(1))))
    return found


async def synthesize(text: str, voice: str, path: Path) -> None:
    import edge_tts  # only needed to generate, not to test the text handling

    for attempt in range(3):
        try:
            await edge_tts.Communicate(text, voice).save(str(path))
            return
        except Exception:  # the online voice can drop a request: retry twice, then fail
            if attempt == 2:
                raise
            await asyncio.sleep(2)


def fingerprint(voice: str, said: str) -> str:
    return hashlib.sha256(f"{voice}\n{said}".encode()).hexdigest()


def previous_audio(out: Path) -> dict[str, bytes]:
    """The audio of the last run by fingerprint, for the files the manifest lists that still exist."""
    manifest = out / "manifest.json"
    if not manifest.exists():
        return {}
    known = {}
    try:
        for entry in json.loads(manifest.read_text(encoding="utf-8")):
            path = out / entry["file"]
            if path.exists():
                known[entry["fingerprint"]] = path.read_bytes()
    except (ValueError, KeyError, TypeError):  # a broken cache (say, merge markers) only costs a full run
        print("manifest.json no se puede leer: se generan todas las slides")
        return {}
    return known


async def generate(slides: Path, out: Path, voice: str) -> None:
    out.mkdir(parents=True, exist_ok=True)
    audio = previous_audio(out)
    plan = []
    with tempfile.TemporaryDirectory() as tmp:
        for number, title, text in notes(slides.read_text(encoding="utf-8")):
            if not text:  # only directions for the presenter: nothing to read, and the voice rejects empty text
                print(f"slide {number:02}: sin texto para leer, se omite")
                continue
            said = pronounce(text)
            key = fingerprint(voice, said)
            state = "reutilizado"
            if key not in audio:
                print(f"slide {number:02}: generando…")
                path = Path(tmp) / f"{key}.mp3"
                await synthesize(said, voice, path)
                audio[key] = path.read_bytes()
                state = "generado"
            plan.append((number, title, text, key, state))

    # Every slide has its audio now, so a failed synthesis above never leaves a half-replaced folder.
    for old in out.glob("slide-*.mp3"):
        old.unlink()
    rows, entries, total = [], [], 0.0
    for number, title, text, key, state in plan:
        name = f"slide-{number:02}.mp3"
        (out / name).write_bytes(audio[key])
        entries.append({"file": name, "fingerprint": key})
        seconds = len(audio[key]) * 8 / BITRATE
        total += seconds
        words = len(re.findall(r"\w+", text))
        rows.append(f"| {number} | {title} | {words} | {seconds:.0f} s | {total / 60:.1f} min | {words / seconds * 60:.0f} |")
        print(f"slide {number:02} {seconds:6.1f} s  total {total / 60:5.1f} min  {state}  {title}")
    table = ["| Slide | Título | Palabras | Duración | Acumulado | Palabras/min |", "|---|---|---|---|---|---|"]
    (out / "duraciones.md").write_text("\n".join([f"Voz: {voice}", "", *table, *rows]) + "\n", encoding="utf-8")
    (out / "manifest.json").write_text(json.dumps(entries, indent=2) + "\n", encoding="utf-8")
    print(f"total {total / 60:.1f} min")


def main() -> None:
    sys.stdout.reconfigure(encoding="utf-8")  # slide titles have accents; the Windows console default mangles them
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--slides", type=Path, default=ROOT / "docs/methodology/slides.md")
    parser.add_argument("--out", type=Path, default=ROOT / "docs/methodology/audio")
    parser.add_argument("--voice", default="es-ES-AlvaroNeural")
    args = parser.parse_args()
    asyncio.run(generate(args.slides, args.out, args.voice))


if __name__ == "__main__":
    main()
