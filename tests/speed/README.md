# Speed comparison with XFOIL

`compare.py` times the same computations in XFOIL 6.996 and in Aero on the
same machine — NACA 2412 paneled with 160 nodes, Re = 10⁶, Ncrit 9: an
inviscid polar, a viscous point from scratch, a viscous polar at prescribed
angles and one at prescribed lift, and the viscous polars of 8 airfoils, one
after the other and on all cores — and writes a Markdown report. The Aero
side is `crates/foil2d/examples/xfoil_speed.rs`.

The `speed-comparison.yml` workflow builds XFOIL from the official sources
(with its own Makefile flags, with `-O3`, and in single precision), builds
Aero in release mode and runs the comparison on one GitHub runner:

```sh
gh workflow run speed-comparison.yml
```

The report is the job summary and the `speed-report` artifact. Locally,
with XFOIL built and an X display (or `xvfb-run`):

```sh
cargo build --release -p aero-foil2d --example xfoil_speed
python3 tests/speed/compare.py --xfoil "local=/path/to/xfoil" \
    --aero target/release/examples/xfoil_speed --repeat 5
```
