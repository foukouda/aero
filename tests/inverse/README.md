# XFOIL inverse design reference data

Reference results of XFOIL 6.996's inverse design methods, used to validate
`aero_foil2d::inverse`. Each `[[inverse]]` run of a case is one XFOIL
session (`inv_<name>.in`, transcript `inv_<name>.log`) on the paneled
airfoil: for each angle of `alphas`, `OPER` `ALFA` then the design menu; the
commands are run at the last visit.

- **Mixed inverse** (`qdes`, XFOIL `QDES`): the target speed distribution is
  the speed at the first angle; at the last angle, the shape of the marked
  segment (`MARK` first and last node, 1-based) is solved for so that the
  speed there matches the target (`EXEC n`: at most n Newton iterations).
  `SMOO` smooths the target on the segment (`SLOP` toggles the slope
  matching at its ends, on by default), `CPXX` adds curvature conditions at
  the ends. The result is the current airfoil, saved by `SAVE`.
- **Full inverse** (`mdes`, XFOIL `MDES`): the shape of the whole airfoil
  follows from the target speed by conformal mapping of a circle. `AQ`/`CQ`
  set the angle or lift of the target, `FILT` applies the Hanning filter to
  the mapping coefficients, `SMOO` smooths the target, `SYMM` forces a
  symmetric airfoil, `TGAP` and `TANG` set the trailing-edge gap and angle.
  The result is the buffer airfoil, copied by `PCOP` and saved by `SAVE`.

XFOIL marks the target segment with the mouse only; the reference build
replaces that input by two keyboard answers (the node indices) with
`patch_iqsget.py`, which the `xfoil-reference` workflow applies before
compiling. Nothing else in XFOIL is changed.

## Regenerating

```sh
python3 tests/inverse/patch_iqsget.py /path/to/Xfoil/src/xqdes.f   # then build XFOIL
python3 tests/reference/generate.py --xfoil /path/to/xfoil --root tests/inverse
```

or `gh workflow run xfoil-reference.yml -f root=tests/inverse`.
