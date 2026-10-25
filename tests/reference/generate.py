#!/usr/bin/env python3
"""Generate XFOIL reference data for the aero validation tests.

Each sub-directory of a case root (``tests/reference`` by default) containing
a ``case.toml`` is one reference case. This script turns every manifest into
XFOIL command files, runs XFOIL on them and leaves XFOIL's raw outputs next to
the manifest.

Inviscid runs (``[[runs]]``), one XFOIL session ``xfoil.in``:

    panels.dat                 PSAV: paneled airfoil nodes
    polar_M<mach>.txt          PACC: inviscid polar (alpha, CL, CDp, CM, Cpmin)
    cp_M<mach>_a<alpha>.txt    CPWR: x, Cp at every panel node
    xfoil.log                  console transcript

Viscous runs (``[[viscous]]``), one fresh XFOIL session ``xfoil_<name>.in``
per run so that every run starts from XFOIL's initial state:

    vpolar_<name>.txt          PACC: viscous polar of the ASEQ sequence
    vdump_<name>.bin           PACC dump file of the sequence (unformatted,
                               full-precision boundary-layer data per point)
    vpoints_<name>.txt         PACC: polar of the cold-started single points
    vpdump_<name>.bin          PACC dump file of the single points
    vcp_<name>_a<alpha>.txt    CPWR after each single point
    vbl_<name>_a<alpha>.txt    DUMP after each single point
    xfoil_<name>.log           console transcript

The sequence starts from an uninitialized boundary layer and ``INIT`` is
issued before each single point (``ALFA``), so every single point is a cold
start, independent of the order of the points.

Usage:
    python3 generate.py --scripts-only          # only write the scripts
    python3 generate.py --xfoil path/to/xfoil   # write scripts and run XFOIL
    python3 generate.py --xfoil xfoil naca0012  # restrict to some cases

The file naming convention is shared with the Rust test harness
(crates/foil2d/tests/common); keep both in sync.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys
import tomllib

HERE = pathlib.Path(__file__).resolve().parent


def polar_name(mach: float) -> str:
    return f"polar_M{mach:.2f}.txt"


def cp_name(mach: float, alpha: float) -> str:
    return f"cp_M{mach:.2f}_a{alpha:+05.1f}.txt"


def viscous_names(run: dict) -> dict:
    name = run["name"]
    if not re.fullmatch(r"[A-Za-z0-9_-]+", name):
        raise SystemExit(f"invalid viscous run name {name!r}")
    return {
        "script": f"xfoil_{name}.in",
        "log": f"xfoil_{name}.log",
        "polar": f"vpolar_{name}.txt",
        "dump": f"vdump_{name}.bin",
        "points": f"vpoints_{name}.txt",
        "pdump": f"vpdump_{name}.bin",
    }


def vcp_name(run: dict, alpha: float) -> str:
    return f"vcp_{run['name']}_a{alpha:+05.1f}.txt"


def vbl_name(run: dict, alpha: float) -> str:
    return f"vbl_{run['name']}_a{alpha:+05.1f}.txt"


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


def geometry_lines(case: dict, save_panels: bool) -> list[str]:
    """Loads the airfoil, panels it and optionally saves the nodes."""
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
    ]
    if save_panels:
        lines.append("PSAV panels.dat")
    return lines


def build_script(case: dict) -> str:
    lines = geometry_lines(case, save_panels=True)
    lines += [
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


def build_viscous_script(case: dict, run: dict, save_panels: bool) -> str:
    names = viscous_names(run)
    lines = geometry_lines(case, save_panels)
    lines += [
        "OPER",
        "CINC",  # include Cpmin in the polar file
        f"VISC {float(run['reynolds']):.1f}",
        f"MACH {float(run.get('mach', 0.0))}",
        "VPAR",
    ]
    ncrit = run.get("ncrit", 9.0)
    if isinstance(ncrit, list):
        lines += [f"NT {float(ncrit[0])}", f"NB {float(ncrit[1])}"]
    else:
        lines.append(f"N {float(ncrit)}")
    xtr = run.get("xtr", [1.0, 1.0])
    lines += [f"XTR {float(xtr[0])} {float(xtr[1])}", ""]  # leave VPAR

    # XFOIL keeps the boundary layer of the previous solution as the initial
    # guess (LBLINI). INIT toggles that flag, so it is only issued once a
    # viscous solution exists.
    initialized = False
    polar = run.get("polar")
    if polar:
        lines += [
            "PACC",
            names["polar"],
            names["dump"],
            f"ASEQ {polar['start']} {polar['end']} {polar['step']}",
            "PACC",
        ]
        initialized = True
    points = run.get("points", [])
    if points:
        lines += ["PACC", names["points"], names["pdump"]]
        for alpha in points:
            if initialized:
                lines.append("INIT")
            lines += [
                f"ALFA {alpha}",
                f"CPWR {vcp_name(run, float(alpha))}",
                f"DUMP {vbl_name(run, float(alpha))}",
            ]
            initialized = True
        lines.append("PACC")

    lines += ["", "QUIT", ""]
    return "\n".join(lines)


def inviscid_outputs(case: dict) -> list[str]:
    names = ["panels.dat", "xfoil.log"]
    for run in case.get("runs", []):
        mach = float(run["mach"])
        if run.get("polar"):
            names.append(polar_name(mach))
        names += [cp_name(mach, float(a)) for a in run.get("cp_alphas", [])]
    return names


def viscous_outputs(run: dict, save_panels: bool) -> list[str]:
    names = viscous_names(run)
    out = [names["log"]]
    if save_panels:
        out.append("panels.dat")
    if run.get("polar"):
        out += [names["polar"], names["dump"]]
    points = run.get("points", [])
    if points:
        out += [names["points"], names["pdump"]]
        for alpha in points:
            out += [vcp_name(run, float(alpha)), vbl_name(run, float(alpha))]
    return out


def sessions(case: dict) -> list[tuple[str, str, str, list[str]]]:
    """XFOIL sessions of a case: (script name, log name, script, outputs)."""
    out = []
    if case.get("runs"):
        out.append(("xfoil.in", "xfoil.log", build_script(case), inviscid_outputs(case)))
    for run in case.get("viscous", []):
        save_panels = not out  # PSAV would prompt before overwriting
        names = viscous_names(run)
        out.append(
            (
                names["script"],
                names["log"],
                build_viscous_script(case, run, save_panels),
                viscous_outputs(run, save_panels),
            )
        )
    return out


def outputs(case: dict) -> list[str]:
    return [name for session in sessions(case) for name in session[3]]


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
        case_sessions = sessions(case)
        for script_name, _, script, _ in case_sessions:
            (case_dir / script_name).write_text(script, newline="\n")
        print(f"[{case_dir.name}] {len(case_sessions)} script(s) written")

        if args.scripts_only:
            continue

        for name in outputs(case) + ["FAILED.txt"]:
            (case_dir / name).unlink(missing_ok=True)
        reason = None
        for script_name, log_name, _, _ in case_sessions:
            try:
                with open(case_dir / script_name) as stdin, open(case_dir / log_name, "w") as log:
                    subprocess.run(
                        [str(pathlib.Path(args.xfoil).resolve())],
                        cwd=case_dir,
                        stdin=stdin,
                        stdout=log,
                        stderr=subprocess.STDOUT,
                        check=True,
                        timeout=900,
                    )
            except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as e:
                reason = f"XFOIL failed on {script_name}: {e}"
                break
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
