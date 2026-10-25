#!/usr/bin/env python3
"""Build the extended validation set: randomly chosen airfoils compared with XFOIL.

The airfoils are drawn at random (fixed seed, so the set is reproducible and
not hand-picked) from:
  - the UIUC Airfoil Coordinates Database (~1600 real-world files, Selig and
    Lednicer formats, often messy);
  - NACA 4-digit and 5-digit designations, open and closed trailing edges;
with random paneling parameters and Mach numbers for part of the cases.

Each case directory gets a case.toml (same format as tests/reference) and,
for UIUC airfoils, the original file plus a cleaned Selig copy written by
this script (independently of the Rust parser) that both XFOIL and aero read.

Usage: python3 build_cases.py [--uiuc 60] [--naca 40] [--seed 20261025]
Then:  python3 ../reference/generate.py --root . --xfoil /path/to/xfoil
"""

from __future__ import annotations

import argparse
import pathlib
import random
import re
import shutil
import sys
import urllib.request

HERE = pathlib.Path(__file__).resolve().parent
UIUC = "https://m-selig.ae.illinois.edu/ads/"


def number_pair(line: str) -> tuple[str, str] | None:
    tokens = [t for t in re.split(r"[\s,;]+", line.strip()) if t]
    if len(tokens) < 2:
        return None
    try:
        float(tokens[0].replace("D", "E").replace("d", "e"))
        float(tokens[1].replace("D", "E").replace("d", "e"))
    except ValueError:
        return None
    return tokens[0], tokens[1]


def clean(text: str) -> list[tuple[str, str]]:
    """Selig-ordered points of a Selig or Lednicer file, digits kept verbatim."""
    rows = [p for p in (number_pair(line) for line in text.splitlines()) if p]
    if not rows:
        raise ValueError("no coordinates")
    a, b = (float(v) for v in rows[0])
    if a >= 2 and b >= 2 and a.is_integer() and b.is_integer():
        nu, nl = int(a), int(b)
        pts = rows[1:]
        if len(pts) != nu + nl:
            raise ValueError(f"Lednicer counts {nu}+{nl} != {len(pts)}")
        upper, lower = pts[:nu], pts[nu:]
        if [float(v) for v in upper[0]] == [float(v) for v in lower[0]]:
            lower = lower[1:]
        rows = list(reversed(upper)) + lower
    # Drop exactly repeated successive points.
    out: list[tuple[str, str]] = []
    for p in rows:
        if out and [float(v) for v in out[-1]] == [float(v) for v in p]:
            continue
        out.append(p)
    # Counterclockwise (Selig) orientation, XFOIL's LOAD area formula.
    xs = [float(p[0]) for p in out]
    ys = [float(p[1]) for p in out]
    area = sum(0.5 * (ys[i] + ys[(i + 1) % len(xs)]) * (xs[i] - xs[(i + 1) % len(xs)]) for i in range(len(xs)))
    if area < 0:
        out.reverse()
    if len(out) < 20:
        raise ValueError(f"only {len(out)} points")
    if len(out) > 1000:
        raise ValueError(f"{len(out)} points (too many for this test)")
    return out


def runs(rng: random.Random) -> str:
    text = """
[[runs]]
mach = 0.0
polar = { start = -6.0, end = 14.0, step = 2.0 }
cp_alphas = [-3.0, 2.0, 7.0]
"""
    if rng.random() < 0.35:
        mach = rng.choice([0.2, 0.3, 0.4, 0.5])
        text += f"""
[[runs]]
mach = {mach}
polar = {{ start = -4.0, end = 6.0, step = 2.0 }}
cp_alphas = [1.0]
"""
    return text


def paneling(rng: random.Random, randomize: bool) -> str:
    if not randomize:
        return "n_nodes = 160\ncvpar = 1.0\ncterat = 0.15\nctrrat = 0.2\n"
    text = (
        f"n_nodes = {rng.choice([60, 100, 140, 200, 280, 360])}\n"
        f"cvpar = {rng.choice([0.3, 0.7, 1.0, 1.5])}\n"
        f"cterat = {rng.choice([0.05, 0.15, 0.3, 0.6])}\n"
        f"ctrrat = {rng.choice([0.1, 0.2, 0.4])}\n"
    )
    if rng.random() < 0.4:
        a = rng.choice([0.1, 0.3, 0.5])
        text += f"xsref = [{a}, {a + 0.3}]\n"
    if rng.random() < 0.3:
        a = rng.choice([0.2, 0.6])
        text += f"xpref = [{a}, {a + 0.2}]\n"
    return text


def write_case(name: str, description: str, airfoil: str, pan: str, run: str) -> None:
    d = HERE / name
    d.mkdir(exist_ok=True)
    (d / "case.toml").write_text(
        "# Extended validation case, built by ../build_cases.py.\n"
        f'description = "{description}"\n\n[airfoil]\n{airfoil}\n[paneling]\n{pan}{run}',
        newline="\n",
    )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--uiuc", type=int, default=60)
    ap.add_argument("--naca", type=int, default=40)
    ap.add_argument("--seed", type=int, default=20261025)
    args = ap.parse_args()
    rng = random.Random(args.seed)

    for old in HERE.iterdir():
        if old.is_dir() and (old / "case.toml").exists():
            shutil.rmtree(old)

    index = urllib.request.urlopen(UIUC + "coord_database.html", timeout=60).read().decode("latin-1")
    files = sorted(set(re.findall(r'href="(coord/[^"]+\.dat)"', index, flags=re.I)))
    print(f"UIUC database: {len(files)} files")
    rng.shuffle(files)

    skipped: list[str] = []
    made = 0
    for rel in files:
        if made >= args.uiuc:
            break
        stem = pathlib.Path(rel).stem
        try:
            raw = urllib.request.urlopen(UIUC + rel, timeout=60).read().decode("latin-1")
            pts = clean(raw)
        except Exception as e:  # noqa: BLE001 - report and skip unusable files
            skipped.append(f"{stem}: {e}")
            continue
        name = "uiuc_" + re.sub(r"[^a-z0-9]+", "_", stem.lower())
        d = HERE / name
        d.mkdir(exist_ok=True)
        (d / "original.dat").write_text(raw.replace("\r\n", "\n"), newline="\n")
        lines = [f"UIUC {stem}"] + [f" {x} {y}" for x, y in pts]
        (d / "airfoil.dat").write_text("\n".join(lines) + "\n", newline="\n")
        write_case(
            name,
            f"UIUC {stem} (random draw)",
            'kind = "file"\nfile = "airfoil.dat"\noriginal = "original.dat"\n',
            paneling(rng, rng.random() < 0.3),
            runs(rng),
        )
        made += 1

    for k in range(args.naca):
        if rng.random() < 0.6:
            m = rng.randint(0, 6)
            p = rng.randint(2, 6) if m else 0
            t = rng.randint(6, 24)
            code = f"{m}{p}{t:02d}"
        else:
            code = f"2{rng.randint(1, 5)}0{rng.randint(8, 21):02d}"
        kind = "naca_sharp" if rng.random() < 0.3 else "naca"
        write_case(
            f"naca_{k:02d}_{code}{'_sharp' if kind == 'naca_sharp' else ''}",
            f"NACA {code} ({'closed' if kind == 'naca_sharp' else 'open'} TE, random draw)",
            f'kind = "{kind}"\ncode = "{code}"\n',
            paneling(rng, True),
            runs(rng),
        )

    print(f"{made} UIUC cases, {args.naca} NACA cases")
    if skipped:
        print(f"{len(skipped)} UIUC files skipped:")
        for s in skipped:
            print("  " + s)
    return 0


if __name__ == "__main__":
    sys.exit(main())
