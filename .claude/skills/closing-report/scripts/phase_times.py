"""Record the phases of an implementation run and report how long each one took.

Usage:
    python -I phase_times.py mark <log> <phase> [--label TEXT] [--start]
    python -I phase_times.py bg-start <log> <phase> <label>
    python -I phase_times.py bg-end <log> <phase> <label>
    python -I phase_times.py end <log>
    python -I phase_times.py report <log> [--lang es|en] [--commits BASE..HEAD]

The log is a tab-separated file with one event per line: local time with offset, event, phase and label.
Every time comes from the clock when the command runs, so the log holds measurements only.

- `mark` closes the current foreground phase and opens <phase>. Only one foreground phase runs at a time,
  so foreground phases add up to the active time. `--start` marks the real start (the user confirmed the
  estimate); the report compares the active time after it with the estimate.
- The `wait` phase is a wait for the user: it is reported apart and never counts as active time.
- `bg-start` / `bg-end` bracket a task that runs in the background while the foreground goes on (for example
  an acceptance run). Background time overlaps the foreground, so it is listed apart and never added.
- `end` closes the current phase. A `mark` after `end` resumes the run; the gap is reported as a pause.
- `report` prints a Markdown summary to stdout.
- `report --commits <base>..<head>` adds the time by commit: each commit gets the active time between the
  previous commit and itself (the first one, everything before it), split by phase, read from `git log`.
  The rows add up to the active time; what happens after the last commit gets its own row.
"""

import argparse
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass, field
from datetime import datetime, timedelta

PHASES = {
    "explore": {"es": "Exploración y mapa", "en": "Exploration and map"},
    "plan": {"es": "Plan (documento de ejecución)", "en": "Planning (execution document)"},
    "implement": {"es": "Código y tests", "en": "Code and tests"},
    "checks": {"es": "Checks (fmt, clippy, suite de tests)", "en": "Checks (fmt, clippy, test suite)"},
    "acceptance": {"es": "Aceptación", "en": "Acceptance"},
    "verify": {"es": "Verificación independiente", "en": "Independent verification"},
    "review": {"es": "Revisión RDD", "en": "RDD review"},
    "fix": {"es": "Correcciones de hallazgos", "en": "Fixes of findings"},
    "docs": {"es": "Documentación", "en": "Documentation"},
    "close": {"es": "Cierre (informe y merge)", "en": "Closing (report and merge)"},
    "wait": {"es": "Espera por el usuario", "en": "Wait for the user"},
}

EVENTS = {"mark", "start", "end", "bg-start", "bg-end"}

LABELS = {
    "es": {
        "phase": "Fase", "duration": "Duración", "share": "% del activo", "spans": "Tramos",
        "total": "Total activo", "begin": "Inicio", "end": "Fin",
        "go": "Arranque confirmado: {at}. Activo desde el arranque: **{since}** (compárese con la estimación); "
              "antes del arranque: {before}.",
        "no_go": "Sin marca de arranque: todo el activo cuenta desde el inicio.",
        "waits": "Esperas por el usuario (fuera del activo): {total}.",
        "no_waits": "Esperas por el usuario: ninguna.",
        "pauses": "Pausas entre sesiones (fuera del activo): {total}.",
        "open": "Fase abierta, no contada: {phase} desde {at}.",
        "background": "En segundo plano (se solapa con las fases de arriba; no suma al activo):",
        "task": "Tarea", "phase_total": "{phase} (total)",
        "bg_open": "Tarea en segundo plano sin cerrar, no contada: {phase} / {label} desde {at}.",
        "by_commit": "Tiempo por commit (cada fila va desde el commit anterior; las esperas no cuentan):",
        "commit": "Commit", "message": "Mensaje", "labels": "Etiquetas", "active": "Activo",
        "breakdown": "Desglose", "after_last": "después del último commit", "commits_total": "Total",
    },
    "en": {
        "phase": "Phase", "duration": "Duration", "share": "% of active", "spans": "Spans",
        "total": "Active total", "begin": "Start", "end": "End",
        "go": "Confirmed start: {at}. Active since the start: **{since}** (compare with the estimate); "
              "before the start: {before}.",
        "no_go": "No start mark: all active time counts from the beginning.",
        "waits": "Waits for the user (outside active time): {total}.",
        "no_waits": "Waits for the user: none.",
        "pauses": "Pauses between sessions (outside active time): {total}.",
        "open": "Open phase, not counted: {phase} since {at}.",
        "background": "In the background (overlaps the phases above; not added to active time):",
        "task": "Task", "phase_total": "{phase} (total)",
        "bg_open": "Background task still open, not counted: {phase} / {label} since {at}.",
        "by_commit": "Time by commit (each row runs from the previous commit; waits do not count):",
        "commit": "Commit", "message": "Message", "labels": "Labels", "active": "Active",
        "breakdown": "Breakdown", "after_last": "after the last commit", "commits_total": "Total",
    },
}


@dataclass
class Event:
    at: datetime
    kind: str
    phase: str
    label: str


@dataclass
class PhaseTime:
    seconds: int = 0
    spans: int = 0


@dataclass
class Summary:
    first: datetime = None
    last: datetime = None
    start: datetime = None
    foreground: dict = field(default_factory=lambda: defaultdict(PhaseTime))
    active: timedelta = timedelta(0)
    active_before_start: timedelta = timedelta(0)
    intervals: list = field(default_factory=list)  # active foreground spans: (phase, begin, end, label)
    waits: list = field(default_factory=list)
    paused: list = field(default_factory=list)
    background: list = field(default_factory=list)
    open_background: list = field(default_factory=list)
    open_phase: tuple = None


def now() -> datetime:
    return datetime.now().astimezone()


def clean(text: str) -> str:
    return " ".join((text or "").split())


def parse_log(text: str):
    events = []
    for number, raw in enumerate(text.splitlines(), start=1):
        if not raw.strip():
            continue
        parts = raw.split("\t")
        try:
            if len(parts) != 4 or parts[1] not in EVENTS or (parts[2] and parts[2] not in PHASES):
                raise ValueError
            moment = datetime.fromisoformat(parts[0])
        except ValueError:
            raise ValueError(f"line {number} is not a phase event: {raw!r}") from None
        if moment.tzinfo is None:
            # Commit times carry an offset; a naive time cannot be compared with them.
            raise ValueError(f"line {number} has a time without UTC offset: {raw!r}")
        events.append(Event(moment, parts[1], parts[2], parts[3]))
    return events


def summarize(events) -> Summary:
    s = Summary()
    current = None  # (phase, opened at, label)
    ended_at = None
    running = {}
    for e in events:
        s.first = s.first or e.at
        s.last = e.at
        if e.kind in ("mark", "start", "end") and current:
            phase, opened, label = current
            spent = e.at - opened
            if phase == "wait":
                s.waits.append((opened, e.at, label))
            else:
                s.intervals.append((phase, opened, e.at, label))
                s.foreground[phase].seconds += int(spent.total_seconds())
                s.foreground[phase].spans += 1
                s.active += spent
                if s.start is None:
                    s.active_before_start += spent
            current = None
        if e.kind in ("mark", "start"):
            if ended_at is not None:
                s.paused.append((ended_at, e.at))
                ended_at = None
            current = (e.phase, e.at, e.label)
            if e.kind == "start":
                s.start = e.at
        elif e.kind == "end":
            ended_at = e.at
        elif e.kind == "bg-start":
            running[(e.phase, e.label)] = e.at
        elif e.kind == "bg-end":
            began = running.pop((e.phase, e.label), None)
            if began is not None:
                s.background.append((e.phase, e.label, began, e.at))
    if current:
        s.open_phase = (current[0], current[1])
    s.open_background = [(phase, label, began) for (phase, label), began in running.items()]
    if s.start is None:
        s.active_before_start = timedelta(0)
    return s


def duration(span: timedelta) -> str:
    total = int(span.total_seconds())
    hours, rest = divmod(total, 3600)
    minutes, seconds = divmod(rest, 60)
    if hours:
        return f"{hours} h {minutes:02d} min"
    return f"{minutes} min {seconds:02d} s"


def clock(moment: datetime) -> str:
    return moment.strftime("%H:%M:%S")


def stamp(moment: datetime) -> str:
    offset = moment.isoformat(timespec="seconds")[19:]  # "+01:00", or "" for a naive time
    return f"{moment:%Y-%m-%d %H:%M:%S} {offset}".rstrip()


def render(s: Summary, lang: str) -> str:
    t = LABELS[lang]
    name = {key: labels[lang] for key, labels in PHASES.items()}
    active_seconds = s.active.total_seconds()
    lines = [f"| {t['phase']} | {t['duration']} | {t['share']} | {t['spans']} |", "|---|---|---|---|"]
    for key in PHASES:
        if key not in s.foreground:
            continue
        p = s.foreground[key]
        share = round(100 * p.seconds / active_seconds) if active_seconds else 0
        lines.append(f"| {name[key]} | {duration(timedelta(seconds=p.seconds))} | {share} % | {p.spans} |")
    spans = sum(p.spans for p in s.foreground.values())
    lines.append(f"| **{t['total']}** | **{duration(s.active)}** | **100 %** | **{spans}** |")
    lines.append("")
    if s.first:
        lines.append(f"- {t['begin']}: {stamp(s.first)}. {t['end']}: {stamp(s.last)}.")
    if s.start:
        lines.append("- " + t["go"].format(
            at=clock(s.start), since=duration(s.active - s.active_before_start),
            before=duration(s.active_before_start),
        ))
    else:
        lines.append("- " + t["no_go"])
    if s.waits:
        total = sum((end - begin for begin, end, _ in s.waits), timedelta(0))
        lines.append("- " + t["waits"].format(total=duration(total)))
        for begin, end, label in s.waits:
            lines.append(f"  - {clock(begin)}–{clock(end)} ({duration(end - begin)})" + (f": {label}" if label else ""))
    else:
        lines.append("- " + t["no_waits"])
    if s.paused:
        total = sum((end - begin for begin, end in s.paused), timedelta(0))
        lines.append("- " + t["pauses"].format(total=duration(total)))
        for begin, end in s.paused:
            lines.append(f"  - {stamp(begin)} – {stamp(end)} ({duration(end - begin)})")
    if s.open_phase:
        lines.append("- " + t["open"].format(phase=name[s.open_phase[0]], at=clock(s.open_phase[1])))
    if s.background:
        lines += ["", t["background"], "",
                  f"| {t['phase']} | {t['task']} | {t['begin']} | {t['end']} | {t['duration']} |",
                  "|---|---|---|---|---|"]
        totals = defaultdict(timedelta)
        for phase, label, begin, end in s.background:
            totals[phase] += end - begin
            lines.append(f"| {name[phase]} | {label} | {clock(begin)} | {clock(end)} | {duration(end - begin)} |")
        for key in PHASES:
            if key in totals:
                label = t["phase_total"].format(phase=name[key])
                lines.append(f"| **{label}** | | | | **{duration(totals[key])}** |")
    for phase, label, began in s.open_background:
        lines.append("")
        lines.append(t["bg_open"].format(phase=name[phase], label=label, at=clock(began)))
    return "\n".join(lines)


@dataclass
class CommitRow:
    sha: str  # None for the row after the last commit
    message: str
    labels: list
    phases: dict  # phase -> active seconds

    @property
    def seconds(self) -> int:
        return sum(self.phases.values())


def by_commit(s: Summary, commits):
    """Split the active foreground time into the spans between consecutive commits.

    `commits` is a list of (sha, committed at, message first line) in history order. A commit dated before
    the previous one gets an empty span, so the spans never overlap and the rows add up to the active time.
    """
    rows = []
    begin = None  # None: from the beginning of the log
    for sha, committed, message in [*commits, (None, None, "")]:
        end = committed if committed is None or begin is None else max(committed, begin)
        phases, labels = {}, []
        for phase, opened, closed, label in s.intervals:
            lo = opened if begin is None else max(opened, begin)
            hi = closed if end is None else min(closed, end)
            seconds = int((hi - lo).total_seconds())
            if seconds <= 0:
                continue
            phases[phase] = phases.get(phase, 0) + seconds
            if label and label not in labels:
                labels.append(label)
        rows.append(CommitRow(sha, message, labels, {k: phases[k] for k in PHASES if k in phases}))
        begin = end
    return rows


def git_commits(revision_range):
    result = subprocess.run(
        ["git", "log", "--reverse", "--format=%h%x09%cI%x09%s", "--end-of-options", revision_range],
        capture_output=True, text=True, encoding="utf-8", errors="replace",
    )
    if result.returncode != 0:
        raise RuntimeError(f"git log failed: {result.stderr.strip()}")
    commits = []
    for raw in result.stdout.splitlines():
        sha, committed, message = raw.split("\t", 2)
        commits.append((sha, datetime.fromisoformat(committed), message))
    return commits


def render_commits(rows, lang: str) -> str:
    t = LABELS[lang]
    name = {key: labels[lang] for key, labels in PHASES.items()}
    lines = [
        t["by_commit"], "",
        f"| {t['commit']} | {t['message']} | {t['labels']} | {t['active']} | {t['breakdown']} |",
        "|---|---|---|---|---|",
    ]
    for row in rows:
        commit = f"`{row.sha}`" if row.sha else "—"
        message = row.message.replace("|", "\\|") if row.sha else t["after_last"]
        breakdown = ", ".join(f"{name[k]} {duration(timedelta(seconds=v))}" for k, v in row.phases.items())
        lines.append(f"| {commit} | {message} | {', '.join(row.labels)} | "
                     f"{duration(timedelta(seconds=row.seconds))} | {breakdown} |")
    total = timedelta(seconds=sum(row.seconds for row in rows))
    lines.append(f"| **{t['commits_total']}** | | | **{duration(total)}** | |")
    return "\n".join(lines)


def parse_args(argv):
    parser = argparse.ArgumentParser(description="Record run phases and report the time of each one.")
    sub = parser.add_subparsers(dest="command", required=True)
    mark = sub.add_parser("mark", help="close the current phase and open another one")
    mark.add_argument("log")
    mark.add_argument("phase", choices=list(PHASES))
    mark.add_argument("--label", default="", help="short note, e.g. the task id")
    mark.add_argument("--start", action="store_true", help="this mark is the real start of the run")
    for name, text in (("bg-start", "a background task starts"), ("bg-end", "a background task ends")):
        bg = sub.add_parser(name, help=text)
        bg.add_argument("log")
        bg.add_argument("phase", choices=list(PHASES))
        bg.add_argument("label", help="names the task; bg-end must repeat it")
    end = sub.add_parser("end", help="close the current phase")
    end.add_argument("log")
    report = sub.add_parser("report", help="print the Markdown summary")
    report.add_argument("log")
    report.add_argument("--lang", choices=sorted(LABELS), default="es", help="report language (default: es)")
    report.add_argument("--commits", metavar="BASE..HEAD", help="add the time by commit for this git range")
    return parser.parse_args(argv)


def read_events(path):
    with open(path, encoding="utf-8") as f:
        return parse_log(f.read())


def append(path, kind, phase="", label=""):
    with open(path, "a", encoding="utf-8", newline="\n") as f:
        f.write(f"{now().isoformat(timespec='seconds')}\t{kind}\t{phase}\t{clean(label)}\n")


def main(argv=None) -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")  # Windows consoles default to cp1252
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        if args.command == "report":
            summary = summarize(read_events(args.log))
            text = render(summary, args.lang)
            if args.commits:
                text += "\n\n" + render_commits(by_commit(summary, git_commits(args.commits)), args.lang)
            print(text)
        elif args.command == "mark":
            append(args.log, "start" if args.start else "mark", args.phase, args.label)
        elif args.command in ("bg-start", "bg-end"):
            label = clean(args.label)
            try:
                open_tasks = summarize(read_events(args.log)).open_background
            except FileNotFoundError:
                open_tasks = []  # bg-start may open the log
            is_open = any(p == args.phase and l == label for p, l, _ in open_tasks)
            if args.command == "bg-start" and is_open:
                print(f"background task {args.phase}/{label} is already open in {args.log}", file=sys.stderr)
                return 2
            if args.command == "bg-end" and not is_open:
                print(f"no open background task {args.phase}/{label} in {args.log}", file=sys.stderr)
                return 2
            append(args.log, args.command, args.phase, label)
        else:
            append(args.log, "end")
    except FileNotFoundError:
        print(f"phase log not found: {args.log}", file=sys.stderr)
        return 2
    except ValueError as error:
        print(f"{args.log}: {error}", file=sys.stderr)
        return 2
    except RuntimeError as error:
        print(error, file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
