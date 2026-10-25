#!/usr/bin/env python3
"""Writes the reference airfoils with corners (doubled points).

A doubled point marks a corner of XFOIL's buffer airfoil: the spline gets a
joint there and PANGEN keeps a node at the corner. These cases exercise
that code path (sharp leading edge, ridges, smooth joints).
"""

import math
import pathlib

HERE = pathlib.Path(__file__).resolve().parent


def naca4(m: float, p: float, t: float, nside: int):
    """NACA 4-digit section with XFOIL's point distribution (naca.f)."""
    an = 1.5
    xs, yt, yc = [], [], []
    for i in range(nside):
        frac = i / (nside - 1)
        x = 1.0 if i == nside - 1 else 1 - (an + 1) * frac * (1 - frac) ** an - (1 - frac) ** (an + 1)
        xs.append(x)
        yt.append((0.2969 * math.sqrt(x) - 0.126 * x - 0.3516 * x**2 + 0.2843 * x**3 - 0.1015 * x**4) * t / 0.2)
        if p == 0:
            yc.append(0.0)
        elif x < p:
            yc.append(m / p**2 * (2 * p * x - x * x))
        else:
            yc.append(m / (1 - p) ** 2 * ((1 - 2 * p) + 2 * p * x - x * x))
    upper = [(xs[i], yc[i] + yt[i]) for i in range(nside - 1, -1, -1)]
    lower = [(xs[i], yc[i] - yt[i]) for i in range(1, nside)]
    return upper, lower


def write(name: str, title: str, points) -> None:
    d = HERE / name
    d.mkdir(exist_ok=True)
    lines = [title] + [f" {x:.10f} {y:.10f}" for x, y in points]
    (d / "airfoil.dat").write_text("\n".join(lines) + "\n", newline="\n")


def main() -> None:
    # Double wedge: straight sides, corners at the nose and at mid-chord.
    k = 30
    t = 0.08
    up_back = [(1 - 0.5 * i / k, t / 2 * i / k) for i in range(k + 1)]
    up_front = [(0.5 - 0.5 * i / k, t / 2 * (1 - i / k)) for i in range(k + 1)]
    low_front = [(0.5 * i / k, -t / 2 * i / k) for i in range(k + 1)]
    low_back = [(0.5 + 0.5 * i / k, -t / 2 * (1 - i / k) - 0.0004 * i / k) for i in range(k + 1)]
    # Each segment end is repeated at the start of the next one: doubled points.
    write("corner_diamond", "DOUBLE WEDGE 8%", up_back + up_front + low_front + low_back)

    # NACA 0012 with a doubled (sharp) leading-edge point.
    upper, lower = naca4(0.0, 0.0, 0.12, 100)
    write("corner_sharp_le", "NACA 0012 SHARP LE", upper + [(0.0, 0.0)] + lower)

    # NACA 4412 with smooth joints (doubled nodes) on both surfaces.
    upper, lower = naca4(0.04, 0.4, 0.12, 100)
    iu = min(range(len(upper)), key=lambda i: abs(upper[i][0] - 0.3))
    il = min(range(len(lower)), key=lambda i: abs(lower[i][0] - 0.6))
    upper.insert(iu, upper[iu])
    lower.insert(il, lower[il])
    write("corner_joints", "NACA 4412 JOINTS", upper + lower)


if __name__ == "__main__":
    main()
