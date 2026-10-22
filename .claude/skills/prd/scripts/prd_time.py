"""Measure how long a PRD took to write and record it in the PRD itself.

The time comes from the Claude Code session history (one JSON object per line, with a timestamp on every
message). Time between two messages belongs to whoever ended the wait:

- a message or answer from the person (a prompt, an AskUserQuestion answer, a permission approval) ends a
  human wait; a human wait longer than --idle-minutes counts only up to that limit and the rest is
  discarded as idle time;
- anything else (model output, tool results, background notifications) is agent work and counts in full.

Run it as the last step of the PRD session, inside Claude Code (it reads CLAUDE_CODE_SESSION_ID):

    python .claude/skills/prd/scripts/prd_time.py docs/prd/x/change.prd.md docs/prd/x/change.prd.en.md

Each PRD gets a "Tiempo de elaboración" / "Elaboration time" section with one row per session; running it
again in the same session replaces that session's row. Exit codes: 0 updated, 2 error (nothing written).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import asdict, dataclass
from datetime import datetime, timedelta
from pathlib import Path

INTERACTIVE_TOOLS = {"AskUserQuestion", "ExitPlanMode"}
DEFAULT_IDLE_MINUTES = 10
HEADINGS = {"es": "## Tiempo de elaboración", "en": "## Elaboration time"}
DATA_PATTERN = re.compile(r"<!-- prd-time: (.*?) -->", re.DOTALL)

LABELS = {
    "es": {
        "columns": ["Sesión", "Inicio", "Reloj", "Descartado", "Activo", "Agente", "Persona", "Intervenciones"],
        "total": "**Total**",
        "note": (
            "Medido con el historial de la sesión de Claude Code. Si la persona tarda más de {idle} min en "
            "responder, se cuentan {idle} min y el resto se descarta como inactividad. Intervenciones: mensajes "
            "y respuestas de la persona después del pedido inicial."
        ),
    },
    "en": {
        "columns": ["Session", "Start", "Wall clock", "Discarded", "Active", "Agent", "Person", "Interventions"],
        "total": "**Total**",
        "note": (
            "Measured from the Claude Code session history. When the person takes more than {idle} min to "
            "reply, {idle} min count and the rest is discarded as idle time. Interventions: messages and "
            "answers from the person after the initial request."
        ),
    },
}


class PrdTimeError(Exception):
    """A problem the user must fix; reported on stderr with exit code 2."""


@dataclass
class Event:
    when: datetime
    human: bool


@dataclass
class Measurement:
    start: datetime
    wall: timedelta
    agent: timedelta
    human: timedelta
    discarded: timedelta
    interventions: int


@dataclass
class SessionRecord:
    """One session's row, stored as JSON in the PRD so later runs can rebuild the table."""

    session: str
    start: str
    wall: int
    discarded: int
    agent: int
    human: int
    interventions: int
    idle_minutes: int


def parse_time(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def is_human(row: dict, tool_names: dict[str, str]) -> bool:
    if row.get("type") != "user" or row.get("isMeta") or row.get("isCompactSummary"):
        return False
    content = row.get("message", {}).get("content")
    if isinstance(content, list):
        results = [item for item in content if isinstance(item, dict) and item.get("type") == "tool_result"]
        if results:
            if any(tool_names.get(item.get("tool_use_id")) in INTERACTIVE_TOOLS for item in results):
                return True
            source = (row.get("permissionDecision") or {}).get("source", "")
            return str(source).startswith("user")
    origin = row.get("origin")
    if isinstance(origin, dict):
        return origin.get("kind") == "human"
    return True


def load_events(path: Path) -> list[Event]:
    rows = []
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            try:
                row = json.loads(line)
            except json.JSONDecodeError:
                continue  # A live session may still be writing its last line.
            if row.get("type") in ("user", "assistant") and row.get("timestamp") and not row.get("isSidechain"):
                rows.append(row)
    tool_names = {}
    for row in rows:
        content = row.get("message", {}).get("content")
        if row["type"] == "assistant" and isinstance(content, list):
            for item in content:
                if isinstance(item, dict) and item.get("type") == "tool_use":
                    tool_names[item.get("id")] = item.get("name")
    return [Event(parse_time(row["timestamp"]), is_human(row, tool_names)) for row in rows]


def measure(events: list[Event], idle_after: timedelta) -> Measurement:
    events = sorted(events, key=lambda event: event.when)
    first = next((index for index, event in enumerate(events) if event.human), None)
    if first is None:
        raise PrdTimeError("the session history has no message from the person")
    events = events[first:]
    agent = human = discarded = timedelta(0)
    interventions = 0
    for previous, current in zip(events, events[1:]):
        gap = current.when - previous.when
        if current.human:
            counted = min(gap, idle_after)
            human += counted
            discarded += gap - counted
            interventions += 1
        else:
            agent += gap
    return Measurement(events[0].when, events[-1].when - events[0].when, agent, human, discarded, interventions)


def to_record(session: str, result: Measurement, idle_minutes: int) -> SessionRecord:
    def seconds(value: timedelta) -> int:
        return int(value.total_seconds())

    return SessionRecord(session, result.start.isoformat(), seconds(result.wall), seconds(result.discarded),
                         seconds(result.agent), seconds(result.human), result.interventions, idle_minutes)


def format_duration(seconds: float) -> str:
    minutes = int(seconds / 60 + 0.5)
    if minutes < 60:
        return f"{minutes} min"
    return f"{minutes // 60} h {minutes % 60} min"


def format_start(value: str) -> str:
    local = parse_time(value).astimezone()
    offset = local.strftime("%z")
    return f"{local:%Y-%m-%d %H:%M} UTC{offset[:3]}:{offset[3:]}"


def render_section(records: list[SessionRecord], lang: str) -> str:
    labels = LABELS[lang]
    lines = [HEADINGS[lang], "", labels["note"].format(idle=records[-1].idle_minutes), ""]
    lines.append("| " + " | ".join(labels["columns"]) + " |")
    lines.append("|" + "---|" * len(labels["columns"]))

    def row(first: str, start: str, wall: int, discarded: int, agent: int, human: int, interventions: int):
        durations = [format_duration(value) for value in (wall, discarded, agent + human, agent, human)]
        return "| " + " | ".join([first, start, *durations, str(interventions)]) + " |"

    for item in records:
        lines.append(row(item.session[:8], format_start(item.start), item.wall, item.discarded, item.agent,
                         item.human, item.interventions))
    if len(records) > 1:
        totals = [sum(getattr(item, name) for item in records)
                  for name in ("wall", "discarded", "agent", "human", "interventions")]
        lines.append(row(labels["total"], "", *totals))
    data = json.dumps([asdict(item) for item in records], ensure_ascii=False)
    lines += ["", f"<!-- prd-time: {data} -->", ""]
    return "\n".join(lines)


def find_section(lines: list[str]) -> tuple[int, int] | None:
    headings = set(HEADINGS.values())
    start = next((index for index, line in enumerate(lines) if line.strip() in headings), None)
    if start is None:
        return None
    end = next((index for index in range(start + 1, len(lines)) if re.match(r"#{1,2} ", lines[index])),
               len(lines))
    return start, end


def update_prd(text: str, record: SessionRecord, lang: str) -> str:
    lines = text.split("\n")
    span = find_section(lines)
    records: list[SessionRecord] = []
    if span:
        match = DATA_PATTERN.search("\n".join(lines[span[0]:span[1]]))
        if match:
            records = [SessionRecord(**item) for item in json.loads(match.group(1))]
    records = [item for item in records if item.session != record.session] + [record]
    section = render_section(records, lang)
    if span:
        before, after = lines[:span[0]], lines[span[1]:]
        return "\n".join(before) + ("\n" if before else "") + section + ("\n" + "\n".join(after) if after else "")
    body = text.rstrip("\n")
    return f"{body}\n\n{section}"


def find_transcript(session: str, config_dir: Path) -> Path:
    matches = sorted((config_dir / "projects").glob(f"*/{session}.jsonl"), key=lambda path: path.stat().st_mtime)
    if not matches:
        raise PrdTimeError(f"no session history for {session} under {config_dir / 'projects'}")
    return matches[-1]


def language_of(path: Path, forced: str | None) -> str:
    if forced:
        return forced
    return "en" if path.name.endswith(".en.md") else "es"


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Record the PRD elaboration time in the PRD itself.")
    parser.add_argument("prd", nargs="+", type=Path, help="PRD files to update (Spanish and translations)")
    parser.add_argument("--session", help="Claude Code session ID (default: CLAUDE_CODE_SESSION_ID)")
    parser.add_argument("--idle-minutes", type=int, default=DEFAULT_IDLE_MINUTES,
                        help=f"longest human wait that still counts (default: {DEFAULT_IDLE_MINUTES})")
    parser.add_argument("--lang", choices=sorted(LABELS), help="section language (default: en for *.en.md)")
    parser.add_argument("--dry-run", action="store_true", help="print the section without writing")
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    try:
        session = args.session or os.environ.get("CLAUDE_CODE_SESSION_ID", "")
        if not session:
            raise PrdTimeError("no session: run inside Claude Code or pass --session <id>")
        missing = [str(path) for path in args.prd if not path.is_file()]
        if missing:
            raise PrdTimeError(f"PRD not found: {', '.join(missing)}")
        config_dir = Path(os.environ.get("CLAUDE_CONFIG_DIR") or Path.home() / ".claude")
        transcript = find_transcript(session, config_dir)
        result = measure(load_events(transcript), timedelta(minutes=args.idle_minutes))
    except PrdTimeError as error:
        print(f"prd_time: {error}", file=sys.stderr)
        return 2
    record = to_record(session, result, args.idle_minutes)
    for path in args.prd:
        raw = path.read_bytes().decode("utf-8")
        newline = "\r\n" if "\r\n" in raw else "\n"
        lang = language_of(path, args.lang)
        updated = update_prd(raw.replace("\r\n", "\n"), record, lang)
        if args.dry_run:
            print(f"--- {path} (dry run)")
        else:
            path.write_bytes(updated.replace("\n", newline).encode("utf-8"))
            print(f"--- {path} updated")
        lines = updated.split("\n")
        start, end = find_section(lines)
        print("\n".join(lines[start:end]).rstrip("\n"))
    return 0


if __name__ == "__main__":
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    sys.exit(main(sys.argv[1:]))
