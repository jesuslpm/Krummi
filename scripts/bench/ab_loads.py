"""A/B of single bench loads between two builds of `krummi-bench` (N11 of the tables PRD: a round of every load before
and after a change, then 10 to 20 rounds of the affected loads and of those more than 3 % apart). Each load runs in a
process of its own through the worker protocol (`KRUMMI_BENCH_ROLE=worker`, `crates/krummi-bench/src/protocol.rs`),
Krummi only, at the full scale; the versions alternate which goes first in each round. A load starts from the base
store, from nothing, or from the last store of the load it follows (`Load::start` in `workload.rs`). Each run keeps the
median ns/op of the measured repetitions 1 to 3; the report gives, per load, the median over the rounds of each version
and their ratio in op/s (above 1, the second version is faster).

Usage: python3 ab_loads.py <work folder> <base krummi-bench> <branch krummi-bench> <rounds> <report> [W1 W2 ...]
"""

import os
import shutil
import statistics
import subprocess
import sys
import time

LOADS = [f"W{i}" for i in range(1, 33) if i != 13]
AFTER = {"W18": "W17", "W20": "W19", "W22": "W21", "W26": "W23", "W27": "W23", "W28": "W23", "W29": "W23",
         "W31": "W30", "W32": "W30"}
EMPTY = {"W1", "W2", "W3", "W17", "W19", "W21", "W23", "W24", "W25", "W30"}


def worker(binary, *args):
    env = dict(os.environ, KRUMMI_BENCH_ROLE="worker")
    done = subprocess.run([binary, *args], capture_output=True, text=True, env=env)
    if done.returncode != 0:
        raise RuntimeError(f"{binary} {' '.join(args)}: {done.stderr.strip()}")
    return done.stdout.splitlines()


def last_rep(store):
    reps = [d for d in os.listdir(store) if d.startswith("rep-")]
    return os.path.join(store, max(reps, key=lambda d: int(d[4:])))


def needed(loads):
    """The loads whose order needs the loads they follow: each load's predecessor runs first."""
    out = []
    for load in loads:
        before = AFTER.get(load)
        if before and before not in out:
            out.append(before)
        if load not in out:
            out.append(load)
    return out


def run_round(binary, folder, loads):
    """ns/op of each load: the median of repetitions 1 to 3."""
    shutil.rmtree(folder, ignore_errors=True)
    os.makedirs(folder)
    base = os.path.join(folder, "base")
    worker(binary, "prepare", "krummi", "full", base)
    results = {}
    for load in needed(loads):
        store = os.path.join(folder, load)
        args = ["run", "krummi", load, "full", store]
        if load in AFTER:
            args.append(last_rep(os.path.join(folder, AFTER[load])))
        elif load not in EMPTY:
            args.append(base)
        for line in worker(binary, *args):
            parts = line.split()
            if parts[:1] == ["rep"] and int(parts[1]) >= 1:
                results.setdefault(parts[2], []).append(int(parts[4]) / int(parts[3]))
        # A store nothing later starts from goes, so the disk holds few at once.
        if load not in AFTER.values():
            shutil.rmtree(store, ignore_errors=True)
    shutil.rmtree(folder, ignore_errors=True)
    return {load: statistics.median(values) for load, values in results.items()}


def main():
    work, base_bin, branch_bin, rounds, report = sys.argv[1:6]
    loads = sys.argv[6:] or LOADS
    shown = loads + (["W13"] if "W12" in loads else [])
    runs = {"base": [], "rama": []}
    started = time.strftime("%Y-%m-%d %H:%M:%S %Z")
    for r in range(int(rounds)):
        order = [("base", base_bin), ("rama", branch_bin)]
        if r % 2:
            order.reverse()
        for name, binary in order:
            runs[name].append(run_round(binary, os.path.join(work, f"{name}-{r}"), loads))
            print(f"ronda {r + 1} {name} hecha", flush=True)
    lines = [
        f"## A/B de una sola carga ({rounds} rondas, {started} a {time.strftime('%H:%M:%S %Z')})\n",
        "| Carga | base ns/op | rama ns/op | op/s rama / base | Rondas de la rama más lentas |",
        "|---|---|---|---|---|",
    ]
    for load in sorted(shown, key=lambda l: int(l[1:])):
        base = [run[load] for run in runs["base"] if load in run]
        branch = [run[load] for run in runs["rama"] if load in run]
        if not base or not branch:
            continue
        b, n = statistics.median(base), statistics.median(branch)
        slower = sum(1 for x in branch if x > max(base))
        lines.append(f"| {load} | {b:,.1f} | {n:,.1f} | {b / n:.3f} | {slower} de {len(branch)} |")
    with open(report, "a", encoding="utf-8") as out:
        out.write("\n".join(lines) + "\n\n")
    print("\n".join(lines))


if __name__ == "__main__":
    main()
