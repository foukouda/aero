# AVL references

Reference results of AVL 3.52 (Mark Drela and Harold Youngren, MIT, GPL)
for the vortex-lattice port of `crates/vlm`. AVL is built from the official
sources (`avl3.52.tgz`, SHA-256 checked) in double precision by the
`avl-reference` workflow, which runs `generate.py` and uploads the outputs;
they are committed here unchanged.

## Cases

| Case | What it exercises |
|---|---|
| `rect` | flat rectangular wing, aspect ratio 10, half model on the `y = 0` plane (`iYsym = 1`), cosine chordwise and −sine spanwise spacing; a pitch rate |
| `tapered` | two-panel wing with sweep, dihedral and washout, NACA 2412 to 0012 camber, whole model (`YDUPLICATE`), Mach 0.3, spacing per section; sideslip and roll, pitch and yaw rates |
| `wing_tail` | E387 wing (`AFILE`) with an interior section matched by the surface spacing, horizontal tail moved by `TRANSLATE` and `ANGLE` with an elevator, fin on the plane of symmetry with a rudder on its default hinge line; three components |
| `flying_wing` | MH 45 reflexed wing with sweep, washout and winglets, sine chordwise spacing, elevons as an elevator and an aileron (`SgnDup` +1 and −1) |
| `canard` | canard with inline `AIRFOIL` coordinates, `SCALE`, `TRANSLATE`, `ANGLE`, `CLAF`; SD7037 wing with an `AFILE` x/c range; a strake sharing the wing's `COMPONENT`, with `NOWAKE` and `NOLOAD` |

Each `case.toml` lists the runs (angle of attack, sideslip, rates `pb/2V`,
`qc/2V`, `rb/2V` about the stability axes, control deflections). Each run is
a fresh AVL session `avl_<run>.in` (`OPER`, the variables, `X`, `MRF`) that
writes, at full precision:

- `ft_<run>.txt`: total forces (`FT`);
- `fs_<run>.txt`: forces of every surface and strip (`FS`);
- `st_<run>.txt`: stability and control derivatives, neutral point (`ST`);
- `avl_<run>.log`: the console transcript.

## Agreement

`crates/vlm/tests/avl_reference.rs` reads each geometry file with the port's
reader, solves the runs and compares every printed number — totals, surface
forces, the 14 columns of every strip, all derivatives — within 10⁻¹⁰ of the
larger of the value and 1. The largest differences are about 10⁻¹² (most are
10⁻¹⁵): the port follows AVL's arithmetic, including its truncated constants
(π in `SPACER`, 1/4π in `VORVELC`); only the LU factorization and the order
of some sums differ.

One printed number is skipped: the centre of pressure of a strip without
lift (the fin in symmetric flight), rounding noise divided by rounding noise.

## A note on AVL's alpha derivatives

With a roll or yaw rate, AVL 3.52's stability-axis derivatives with respect
to the angle of attack turn those rates with alpha with the opposite sign
(`WROT_A` in `DERMATS` uses the rates already multiplied by `DIR = −1`). The
port keeps AVL's values, so that they match; at zero roll and yaw rates —
steady flight, where the derivatives are used — there is no difference, and
`crates/vlm/tests/theory.rs` checks every derivative against finite
differences there.

## Regenerating

Push the cases to a branch named `avl-reference-*` (or run the workflow by
hand), download the `avl-outputs` artifact, review and commit it:

```sh
gh run download <run-id> -n avl-outputs -D tests/avl
```
