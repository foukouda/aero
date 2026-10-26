#!/usr/bin/env python3
"""Generate AVL reference data for the vortex-lattice tests of aero-vlm.

Each sub-directory of ``tests/avl`` containing a ``case.toml`` is one
reference case: an AVL geometry file (``geometry``, read by AVL and by the
Rust test harness) and a list of runs. Every run is one fresh AVL session
``avl_<name>.in``, so that runs do not depend on each other:

    OPER, alpha (A A), beta (B B), stability-axis rates pb/2V, qc/2V, rb/2V
    (R R, P P, Y Y), control deflections in degrees (D1 D1, D2 D2, ...),
    X (execute), MRF (full-precision machine-readable output), then

    ft_<name>.txt     FT: total forces
    fs_<name>.txt     FS: surface and strip forces
    st_<name>.txt     ST: stability-axis derivatives and neutral point
    avl_<name>.log    console transcript

Keys of a run: ``name``, ``alpha``, ``beta``, ``roll``, ``pitch``, ``yaw``
(degrees and normalized rates, 0 by default), ``controls`` (deflections in
degrees, in the order the controls are declared in the geometry file).

Usage:
    python3 generate.py --scripts-only        # only write the sessions
    python3 generate.py --avl path/to/avl     # write and run them
    python3 generate.py --avl avl rect        # restrict to some cases

The file names are shared with the Rust harness
(crates/vlm/tests/avl_reference.rs); keep both in sync.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys
import tomllib

HERE = pathlib.Path(__file__).resolve().parent


def session(run: dict) -> str:
    name = run["name"]
    if not re.fullmatch(r"[A-Za-z0-9_.+-]+", name):
        raise SystemExit(f"invalid run name {name!r}")
    lines = [
        "OPER",
        f"A A {float(run.get('alpha', 0.0))!r}",
        f"B B {float(run.get('beta', 0.0))!r}",
        f"R R {float(run.get('roll', 0.0))!r}",
        f"P P {float(run.get('pitch', 0.0))!r}",
        f"Y Y {float(run.get('yaw', 0.0))!r}",
    ]
    for k, delta in enumerate(run.get("controls", []), start=1):
        lines.append(f"D{k} D{k} {float(delta)!r}")
    lines += [
        "X",
        "MRF",
        f"FT ft_{name}.txt",
        f"FS fs_{name}.txt",
        f"ST st_{name}.txt",
        "",  # leave OPER
        "QUIT",
    ]
    return "\n".join(lines) + "\n"


def run_case(case_dir: pathlib.Path, avl: str | None) -> bool:
    manifest = tomllib.loads((case_dir / "case.toml").read_text())
    geometry = manifest["geometry"]
    ok = True
    for run in manifest["runs"]:
        name = run["name"]
        script = case_dir / f"avl_{name}.in"
        script.write_text(session(run), newline="\n")
        if avl is None:
            continue
        for prefix in ("ft", "fs", "st"):
            (case_dir / f"{prefix}_{name}.txt").unlink(missing_ok=True)
        with open(script) as stdin, open(case_dir / f"avl_{name}.log", "w") as log:
            result = subprocess.run(
                [avl, geometry], cwd=case_dir, stdin=stdin, stdout=log,
                stderr=subprocess.STDOUT, timeout=600,
            )
        missing = [
            p for p in ("ft", "fs", "st") if not (case_dir / f"{p}_{name}.txt").exists()
        ]
        if result.returncode != 0 or missing:
            print(f"{case_dir.name}/{name}: AVL exit {result.returncode}, missing {missing}")
            ok = False
        else:
            print(f"{case_dir.name}/{name}: ok")
    return ok


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--avl", help="AVL executable")
    parser.add_argument("--scripts-only", action="store_true")
    parser.add_argument("cases", nargs="*", help="case directories (default: all)")
    args = parser.parse_args()
    if not args.scripts_only and not args.avl:
        parser.error("--avl or --scripts-only is required")

    dirs = sorted(p.parent for p in HERE.glob("*/case.toml"))
    if args.cases:
        dirs = [d for d in dirs if d.name in args.cases]
    ok = all([run_case(d, None if args.scripts_only else args.avl) for d in dirs])
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
