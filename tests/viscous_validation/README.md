# Extended viscous validation set

Fifty viscous analyses drawn at random by [`build_cases.py`](build_cases.py)
(fixed seed): the airfoil (30 UIUC airfoils of [`../validation`](../validation),
20 NACA 4- and 5-digit sections with open or closed trailing edges), the
paneling, the Reynolds number (10⁵ to 10⁷), Ncrit (the same or different on
both sides), trips, the Mach number, the polar range and two single points.

The case and output formats are those of [`../viscous`](../viscous/README.md).
The outputs are produced by XFOIL 6.996 in CI:

```sh
gh workflow run xfoil-reference.yml -f root=tests/viscous_validation
```

`crates/foil2d/tests/xfoil_viscous_validation.rs` compares them with aero;
set `AERO_VALIDATION_REPORT=1` to write `REPORT.md`.
