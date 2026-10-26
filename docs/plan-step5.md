# Step 5 — geometry design tools and inverse design

Requested on 2026-10-26 ("1" of the list of what was missing): the XFOIL
tools that modify an airfoil — flap deflection and the other geometry
commands of `GDES`, interpolation between airfoils (`INTE`) — and the two
inverse design methods: mixed inverse (`QDES`) and full inverse (`MDES`).
Each routine is ported line by line, as for the solvers, and checked
against XFOIL 6.996.

## G1 — geometry modifications (`aero_geometry::modify`)

| Operation | XFOIL | Routines |
|---|---|---|
| Flap deflection about a hinge (x, y or y/t) | `GDES` `FLAP` | `FLAP`, `SSS`, `INSIDE`, `SCHECK`, `GETXYF` |
| Trailing-edge gap, blended over a distance | `GDES` `TGAP` | `TGAP` |
| Leading-edge radius scaling | `GDES` `LERA` | `LERAD`, `LERSCL` |
| Scale thickness and camber | `GDES` `TFAC` | `TCSCAL`, `THKCAM` |
| Set maximum thickness and camber | `GDES` `TSET` | `TCSET`, `GETCAM`, `GETMAX`, `SORTOL` |
| Move the thickness and camber high points | `GDES` `HIGH` | `HIPNT`, `XLFIND`, `SPLINA` |
| Interpolate (or extrapolate) two airfoils | `INTE` | `INTE`, `INTER` |

References: XFOIL sessions applying these commands (`GDES`, `EXEC`,
`SAVE`, 7 significant digits) on NACA and UIUC airfoils, and polars of
flapped airfoils (inviscid and viscous) through the existing harness.

## G2 — application: modifying airfoils

A "Modify" panel in the Geometry tab: each operation with its parameters,
a preview over the original shape, and "Create" adds the result to the
library (the original is kept). A flap series creates several deflections
at once (e.g. 0, 5, 10, 15°), ready to be swept and compared together.

## I1 — mixed inverse (`QDES`)

The surface speed is prescribed on a segment of the surface and the shape
of that segment is solved for, the rest of the airfoil being kept:
`PSILIN` with geometric sensitivities (`GEOLIN`, not ported in step 1),
`GAMQSP`, `SPLQSP`, `SMOOQ`, `SYMQSP`, `MIXED`, `GAMLIN`. References: XFOIL
marks the target segment with the mouse only, so the reference build gets
a three-line patch of `IQSGET` reading the two endpoint indices from the
keyboard (input only; the computation is untouched); sessions `MARK`,
`SMOO`, `EXEC`.

## I2 — full inverse (`MDES`)

The surface speed is prescribed on the whole airfoil at one or several
angles or lift coefficients and the shape is obtained by conformal mapping
of a circle (Fourier coefficients Cn, Lighthill constraints, trailing-edge
gap and angle): `EIWSET`, `FTP`, `SCINIT`, `CNCALC`, `CNSYMM`, `PIQSUM`,
`CNFILT`, `ZCCALC`, `ZCNORM`, `ZLEFIND`, `QCCALC`, `QSPINT`, `MAPGAM`,
`QSPCIR`, `MAPGEN`, `DZTSET`, `AGTSET`, `GETVOV`. References: sessions
`AQ`/`CQ`, `FILT`, `SMOO`, `SYMM`, `TGAP`, `TANG`, `READ` (a target
distribution from a file), `EXEC`.

## I3 — application: inverse design

An "Inverse design" view: the speed (or Cp) distribution of the selected
airfoil at a chosen α or Cl; the target is modified by drawing over the
curve (a spline through the drawn points, blended into the current
distribution, as XFOIL's `MODI`), smoothing or filtering; mixed inverse on
the modified segment or full inverse; preview of the new shape and of its
Cp; "Add to library".

## D — documentation

Glossary entries (flap, inverse design, target distribution), guided tour,
README, release 0.2.0.

## Status

Done on 2026-10-26: G1 (17 XFOIL geometry edits matched to 8·10⁻⁸, flapped
polars), G2, I1 (7 `QDES` sessions), I2 (6 `MDES` sessions; the reference
patch of `IQSGET` is in `tests/inverse/patch_iqsget.py`), I3 (Inverse
design tab, curves drawn with the mouse), D (glossary, tour, README,
release 0.2.0).
