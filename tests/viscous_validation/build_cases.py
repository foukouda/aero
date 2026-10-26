#!/usr/bin/env python3
"""Build the extended viscous validation set: random viscous analyses compared with XFOIL.

Everything is drawn at random with a fixed seed (reproducible, not
hand-picked):
  - the airfoil: one of the UIUC airfoils of ../validation (cleaned Selig
    files that XFOIL and aero both read) or a NACA 4/5-digit section with an
    open or closed trailing edge;
  - the paneling (mostly XFOIL's defaults, sometimes other node counts);
  - the Reynolds number (1e5 to 1e7), the critical amplification ratio (the
    same or different on both sides), trips on part of the cases, the Mach
    number on part of the cases;
  - the polar range and step, and two single points.

Each case directory gets a case.toml in the format of ../viscous.

Usage: python3 build_cases.py [--uiuc 30] [--naca 20] [--seed 20261026]
Then:  python3 ../reference/generate.py --root . --xfoil /path/to/xfoil --keep-going
"""

from __future__ import annotations

import argparse
import pathlib
import random
import shutil
import sys

HERE = pathlib.Path(__file__).resolve().parent
UIUC_SOURCE = HERE.parent / "validation"


def paneling(rng: random.Random) -> str:
    if rng.random() < 0.7:
        return "n_nodes = 160\ncvpar = 1.0\ncterat = 0.15\nctrrat = 0.2\n"
    n = rng.choice([120, 200, 240, 280])
    cterat = rng.choice([0.15, 0.3])
    return f"n_nodes = {n}\ncvpar = 1.0\ncterat = {cterat}\nctrrat = 0.2\n"


def viscous_run(rng: random.Random) -> str:
    reynolds = rng.choice([1e5, 2e5, 3e5, 5e5, 1e6, 2e6, 3e6, 6e6, 1e7])
    mach = rng.choice([0.1, 0.2, 0.3, 0.4]) if rng.random() < 0.25 else 0.0
    if rng.random() < 0.7:
        ncrit = f"{rng.choice([5.0, 7.0, 9.0, 11.0])}"
    else:
        ncrit = f"[{rng.choice([4.0, 6.0, 9.0])}, {rng.choice([8.0, 10.0, 12.0])}]"
    if rng.random() < 0.3:
        xtr = [rng.choice([0.02, 0.05, 0.1, 0.3, 0.5, 0.8, 1.0]) for _ in range(2)]
    else:
        xtr = [1.0, 1.0]
    start = rng.choice([-6.0, -4.0, -2.0])
    end = rng.choice([8.0, 10.0, 12.0, 14.0, 16.0])
    step = rng.choice([1.0, 2.0])
    points = sorted({float(rng.randint(-4, 12)) for _ in range(2)})
    return f"""
[[viscous]]
name = "run"
reynolds = {reynolds}
mach = {mach}
ncrit = {ncrit}
xtr = [{xtr[0]}, {xtr[1]}]
polar = {{ start = {start}, end = {end}, step = {step} }}
points = [{", ".join(str(p) for p in points)}]
"""


def write_case(name: str, description: str, airfoil: str, rng: random.Random) -> pathlib.Path:
    d = HERE / name
    d.mkdir(exist_ok=True)
    (d / "case.toml").write_text(
        "# Extended viscous validation case, built by ../build_cases.py.\n"
        f'description = "{description}"\n\n[airfoil]\n{airfoil}\n[paneling]\n'
        f"{paneling(rng)}{viscous_run(rng)}",
        newline="\n",
    )
    return d


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--uiuc", type=int, default=30)
    ap.add_argument("--naca", type=int, default=20)
    ap.add_argument("--seed", type=int, default=20261026)
    args = ap.parse_args()
    rng = random.Random(args.seed)

    for old in HERE.iterdir():
        if old.is_dir() and (old / "case.toml").exists():
            shutil.rmtree(old)

    uiuc = sorted(p.parent for p in UIUC_SOURCE.glob("uiuc_*/airfoil.dat"))
    rng.shuffle(uiuc)
    for src in uiuc[: args.uiuc]:
        d = write_case(
            src.name,
            f"{src.name} of ../validation (random draw)",
            'kind = "file"\nfile = "airfoil.dat"\n',
            rng,
        )
        shutil.copyfile(src / "airfoil.dat", d / "airfoil.dat")

    for k in range(args.naca):
        if rng.random() < 0.6:
            m = rng.randint(0, 6)
            p = rng.randint(2, 6) if m else 0
            t = rng.randint(6, 21)
            code = f"{m}{p}{t:02d}"
        else:
            code = f"2{rng.randint(1, 5)}0{rng.randint(8, 18):02d}"
        kind = "naca_sharp" if rng.random() < 0.3 else "naca"
        write_case(
            f"naca_{k:02d}_{code}{'_sharp' if kind == 'naca_sharp' else ''}",
            f"NACA {code} ({'closed' if kind == 'naca_sharp' else 'open'} TE, random draw)",
            f'kind = "{kind}"\ncode = "{code}"\n',
            rng,
        )

    print(f"{min(args.uiuc, len(uiuc))} UIUC cases, {args.naca} NACA cases")
    return 0


if __name__ == "__main__":
    sys.exit(main())
