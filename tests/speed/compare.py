#!/usr/bin/env python3
"""Speed comparison of Aero and XFOIL 6.996 on the same machine.

Runs the same tasks — NACA 2412 paneled with 160 nodes (PPAR N 160, P 1,
T 0.15, R 0.2), Re = 1e6, Ncrit 9 — in one or more XFOIL builds and in
Aero (`crates/foil2d/examples/xfoil_speed.rs`), and writes a Markdown
report.

XFOIL is timed as a whole session (start, NACA, PANE, OPER, the task,
QUIT) minus an empty session (start, NACA, PANE, OPER, QUIT), so that only
the task is counted, like Aero's timings, which start from the paneled
airfoil. The batch task (viscous polars of 8 airfoils) counts whole
sessions, one after the other and with one process per core, against
Aero computing the same polars on one thread and on all cores.

Usage (XFOIL needs an X display even with graphics off: use xvfb-run):

    xvfb-run -a python3 tests/speed/compare.py \\
        --xfoil "official (gfortran -O, double)=/path/to/xfoil" \\
        --aero target/release/examples/xfoil_speed --repeat 5 --out report.md
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import pathlib
import platform
import statistics
import subprocess
import sys
import tempfile
import time

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "reference"))
from generate import geometry_lines  # noqa: E402  (same session prologue as the references)

PANELING = {"n_nodes": 160, "cvpar": 1.0, "cterat": 0.15, "ctrrat": 0.2}
BATCH = ["0012", "2412", "4412", "4415", "23012", "6409", "0009", "2415"]
VISC = ["VISC 1000000"]  # Ncrit 9, free transition and ITER 20 are XFOIL's defaults
TASKS = {
    "inviscid_polar": ["ASEQ -4 14 0.5"],
    "viscous_point": VISC + ["ALFA 4"],
    "viscous_polar": VISC + ["ASEQ -4 14 0.5"],
    "viscous_cl_polar": VISC + ["CSEQ 0 1.2 0.1"],
}
LABELS = {
    "inviscid_polar": "Inviscid polar, 37 angles (−4° to 14°)",
    "viscous_point": "Viscous point from scratch (α = 4°)",
    "viscous_polar": "Viscous polar, 37 angles (−4° to 14°)",
    "viscous_cl_polar": "Viscous polar at prescribed Cl, 13 values (0 to 1.2)",
    "batch_sequential": "8 airfoils × viscous polar, one core",
    "batch_parallel": "8 airfoils × viscous polar, all cores",
}


def session(code: str, commands: list[str]) -> str:
    case = {"airfoil": {"kind": "naca", "code": code}, "paneling": PANELING}
    lines = geometry_lines(case, save_panels=False) + ["OPER"] + commands + ["", "QUIT", ""]
    return "\n".join(lines)


def run_xfoil(xfoil: str, script: str, workdir: str, capture: bool = False) -> tuple[float, str]:
    start = time.perf_counter()
    done = subprocess.run(
        [xfoil],
        input=script,
        text=True,
        cwd=workdir,
        stdout=subprocess.PIPE if capture else subprocess.DEVNULL,
        stderr=subprocess.STDOUT,
        timeout=900,
        check=True,
    )
    return time.perf_counter() - start, done.stdout or ""


def median_time(repeat: int, run) -> float:
    return statistics.median(run() for _ in range(repeat))


def xfoil_results(xfoil: str, repeat: int, workers: int) -> dict:
    out: dict[str, dict] = {}
    with tempfile.TemporaryDirectory() as workdir:
        empty = session("2412", [])
        base = median_time(repeat, lambda: run_xfoil(xfoil, empty, workdir)[0])
        out["startup"] = {"seconds": base}
        for task, commands in TASKS.items():
            script = session("2412", commands)
            _, log = run_xfoil(xfoil, script, workdir, capture=True)
            total = median_time(repeat, lambda: run_xfoil(xfoil, script, workdir)[0])
            out[task] = {
                "seconds": max(total - base, 0.0),
                "failed": log.count("Convergence failed"),
            }
        scripts = [session(code, VISC + ["ASEQ -4 14 0.5"]) for code in BATCH]

        def sequential() -> float:
            start = time.perf_counter()
            for s in scripts:
                run_xfoil(xfoil, s, workdir)
            return time.perf_counter() - start

        def parallel() -> float:
            start = time.perf_counter()
            with concurrent.futures.ThreadPoolExecutor(max_workers=workers) as pool:
                list(pool.map(lambda s: run_xfoil(xfoil, s, workdir), scripts))
            return time.perf_counter() - start

        out["batch_sequential"] = {"seconds": median_time(repeat, sequential)}
        out["batch_parallel"] = {"seconds": median_time(repeat, parallel)}
    return out


def aero_results(binary: str, repeat: int, workers: int) -> dict:
    done = subprocess.run(
        [binary, "--repeat", str(repeat), "--threads", str(workers)],
        text=True,
        capture_output=True,
        check=True,
        timeout=3600,
    )
    rows = [json.loads(line) for line in done.stdout.splitlines() if line.startswith("{")]
    return {r["task"]: r for r in rows}


def cpu_name() -> str:
    try:
        for line in pathlib.Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or "unknown CPU"


def ms(seconds: float) -> str:
    return f"{seconds * 1000:.1f} ms" if seconds < 1 else f"{seconds:.2f} s"


def report(xfoils: dict[str, dict], aero: dict, repeat: int, workers: int) -> str:
    names = list(xfoils)
    lines = [
        "# Speed: Aero compared with XFOIL 6.996",
        "",
        f"Same machine: {cpu_name()}, {os.cpu_count()} logical cores, "
        f"{platform.system()} {platform.release()}. Median of {repeat} runs. "
        "NACA 2412 paneled with 160 nodes, Re = 10⁶, Ncrit 9; the batch uses 8 NACA "
        f"sections ({', '.join(BATCH)}) and {workers} worker(s).",
        "",
        "XFOIL times are whole sessions minus an empty session (start-up, NACA, PANE: "
        + ", ".join(f"{ms(x['startup']['seconds'])} for {n}" for n, x in xfoils.items())
        + "), except the batch rows, which count whole sessions (start-up included), as "
        "running XFOIL on many airfoils does. Aero times start from the paneled airfoil; "
        "the batch rows include the geometry and the paneling.",
        "",
        "| Task | " + " | ".join(f"XFOIL {n}" for n in names) + " | Aero | Aero faster by |",
        "|---|" + "---|" * (len(names) + 2),
    ]
    for task, label in LABELS.items():
        a = aero[task]["seconds"]
        xs = [x[task]["seconds"] for x in xfoils.values()]
        fastest = min(xs)
        ratio = f"× {fastest / a:.1f}" if a > 0 else "—"
        lines.append(
            f"| {label} | " + " | ".join(ms(v) for v in xs) + f" | {ms(a)} | {ratio} |"
        )
    lines += [
        "",
        "“Aero faster by” compares with the fastest XFOIL build of the row.",
        "",
        "Points that did not converge (same work on both sides):",
        "",
        "| Task | " + " | ".join(f"XFOIL {n}" for n in names) + " | Aero |",
        "|---|" + "---|" * (len(names) + 1),
    ]
    for task in TASKS:
        lines.append(
            f"| {LABELS[task]} | "
            + " | ".join(str(x[task]["failed"]) for x in xfoils.values())
            + f" | {aero[task]['failed']} |"
        )
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--xfoil",
        action="append",
        required=True,
        help="LABEL=PATH of an XFOIL executable (repeatable)",
    )
    parser.add_argument("--aero", required=True, help="path of the xfoil_speed example binary")
    parser.add_argument("--repeat", type=int, default=5)
    parser.add_argument("--workers", type=int, default=os.cpu_count() or 1)
    parser.add_argument("--out", default="speed-report.md")
    parser.add_argument("--json", default="speed-results.json")
    args = parser.parse_args()

    xfoils = {}
    for spec in args.xfoil:
        label, _, path = spec.partition("=")
        print(f"timing XFOIL {label} ...", flush=True)
        xfoils[label] = xfoil_results(str(pathlib.Path(path).resolve()), args.repeat, args.workers)
    print("timing Aero ...", flush=True)
    aero = aero_results(args.aero, args.repeat, args.workers)
    text = report(xfoils, aero, args.repeat, args.workers)
    pathlib.Path(args.out).write_text(text, encoding="utf-8")
    pathlib.Path(args.json).write_text(
        json.dumps({"xfoil": xfoils, "aero": aero}, indent=2), encoding="utf-8"
    )
    print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
