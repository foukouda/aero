#!/usr/bin/env python3
"""Generate XFOIL reference data for the aero validation tests.

Each sub-directory of ``tests/reference`` containing a ``case.toml`` is one
reference case. This script turns every manifest into an XFOIL command file
(``xfoil.in``), runs XFOIL on it and leaves XFOIL's raw outputs next to the
manifest:

    panels.dat                 PSAV: paneled airfoil nodes
    polar_M<mach>.txt          PACC: inviscid polar (alpha, CL, CDp, CM, Cpmin)
    cp_M<mach>_a<alpha>.txt    CPWR: x, Cp at every panel node
    xfoil.log                  console transcript

Usage:
    python3 generate.py --scripts-only          # only write xfoil.in files
    python3 generate.py --xfoil path/to/xfoil   # write scripts and run XFOIL
    python3 generate.py --xfoil xfoil naca0012  # restrict to some cases

The file naming convention is shared with the Rust test harness
(crates/foil2d/tests/xfoil_reference.rs); keep both in sync.
"""

from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys
import tomllib

HERE = pathlib.Path(__file__).resolve().parent


def polar_name(mach: float) -> str:
    return f"polar_M{mach:.2f}.txt"


def cp_name(mach: float, alpha: float) -> str:
    return f"cp_M{mach:.2f}_a{alpha:+05.1f}.txt"


def lednicer_to_selig(src: pathlib.Path, dst: pathlib.Path) -> None:
    """Convert a Lednicer file to Selig ordering, keeping the digits verbatim."""
    lines = src.read_text().splitlines()
    name = lines[0].strip()
    counts = lines[1].split()
    n_upper, n_lower = int(float(counts[0])), int(float(counts[1]))
    points = [ln.split() for ln in lines[2:] if len(ln.split()) >= 2]
    if len(points) != n_upper + n_lower:
        raise SystemExit(f"{src}: expected {n_upper + n_lower} points, found {len(points)}")
    upper = points[:n_upper]
    lower = points[n_upper:]
    if upper[0] == lower[0]:
        lower = lower[1:]  # shared leading-edge point
    selig = list(reversed(upper)) + lower
    out = [name] + [f" {x} {y}" for x, y in selig]
    dst.write_text("\n".join(out) + "\n", newline="\n")


def build_script(case: dict) -> str:
    lines: list[str] = ["PLOP", "G", ""]  # disable graphics

    airfoil = case["airfoil"]
    kind = airfoil["kind"]
    if kind == "naca":
        lines.append(f"NACA {airfoil['code']}")
    elif kind == "naca_sharp":
        lines.append(f"NACS {airfoil['code']}")
    elif kind == "file":
        lines.append(f"LOAD {airfoil['file']}")
    else:
        raise SystemExit(f"unknown airfoil kind {kind!r}")

    pan = case["paneling"]
    lines += [
        "PPAR",
        f"N {pan['n_nodes']}",
        f"P {pan['cvpar']}",
        f"T {pan['cterat']}",
        f"R {pan['ctrrat']}",
    ]
    if "xsref" in pan:
        lines.append(f"XT {pan['xsref'][0]} {pan['xsref'][1]}")
    if "xpref" in pan:
        lines.append(f"XB {pan['xpref'][0]} {pan['xpref'][1]}")
    lines += [
        "",  # leave PPAR: re-panels with the new parameters
        "",  # leave the redisplayed PPAR menu
        "PSAV panels.dat",
        "OPER",
        "CINC",  # include Cpmin in the polar file
    ]

    for run in case["runs"]:
        mach = float(run["mach"])
        lines.append(f"MACH {mach}")
        polar = run.get("polar")
        if polar:
            lines += [
                "PACC",
                polar_name(mach),
                "",  # no polar dump file
                f"ASEQ {polar['start']} {polar['end']} {polar['step']}",
                "PACC",
            ]
        for alpha in run.get("cp_alphas", []):
            lines.append(f"ALFA {alpha}")
            lines.append(f"CPWR {cp_name(mach, float(alpha))}")

    lines += ["", "QUIT", ""]
    return "\n".join(lines)


def outputs(case: dict) -> list[str]:
    names = ["panels.dat", "xfoil.log"]
    for run in case["runs"]:
        mach = float(run["mach"])
        if run.get("polar"):
            names.append(polar_name(mach))
        names += [cp_name(mach, float(a)) for a in run.get("cp_alphas", [])]
    return names


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--xfoil", help="path to the XFOIL executable")
    parser.add_argument("--scripts-only", action="store_true")
    parser.add_argument("--root", default=str(HERE), help="directory holding the cases")
    parser.add_argument(
        "--keep-going",
        action="store_true",
        help="record failing cases in FAILED.txt instead of stopping",
    )
    parser.add_argument("cases", nargs="*", help="case directories (default: all)")
    args = parser.parse_args()

    if not args.scripts_only and not args.xfoil:
        parser.error("either --xfoil or --scripts-only is required")

    root = pathlib.Path(args.root).resolve()
    dirs = sorted(p.parent for p in root.glob("*/case.toml"))
    failed: list[str] = []
    if args.cases:
        dirs = [d for d in dirs if d.name in args.cases]

    for case_dir in dirs:
        case = tomllib.loads((case_dir / "case.toml").read_text())
        lednicer = case["airfoil"].get("lednicer_source")
        if lednicer:
            lednicer_to_selig(case_dir / lednicer, case_dir / case["airfoil"]["file"])
        (case_dir / "xfoil.in").write_text(build_script(case), newline="\n")
        print(f"[{case_dir.name}] script written")

        if args.scripts_only:
            continue

        for name in outputs(case) + ["FAILED.txt"]:
            (case_dir / name).unlink(missing_ok=True)
        reason = None
        try:
            with open(case_dir / "xfoil.in") as stdin, open(case_dir / "xfoil.log", "w") as log:
                subprocess.run(
                    [str(pathlib.Path(args.xfoil).resolve())],
                    cwd=case_dir,
                    stdin=stdin,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    check=True,
                    timeout=120,
                )
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as e:
            reason = f"XFOIL failed: {e}"
        missing = [n for n in outputs(case) if not (case_dir / n).exists()]
        if missing and reason is None:
            reason = f"missing outputs: {missing}"
        if reason:
            print(f"[{case_dir.name}] {reason}", file=sys.stderr)
            if not args.keep_going:
                return 1
            (case_dir / "FAILED.txt").write_text(reason + "\n", newline="\n")
            failed.append(case_dir.name)
            continue
        print(f"[{case_dir.name}] done")
    if failed:
        print(f"{len(failed)} case(s) failed in XFOIL: {', '.join(failed)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
