# XFOIL viscous reference data

Reference results of XFOIL 6.996 viscous analyses (boundary layer, transition,
drag), used to validate `aero_foil2d::viscous`. Every sub-directory with a
`case.toml` is one case; the outputs are produced by
[`../reference/generate.py`](../reference/generate.py) with the same XFOIL
build as the inviscid references (see [`../reference/README.md`](../reference/README.md)
for the provenance).

| Case | Airfoil | Re | Mach | Ncrit | Trips | Paneling |
|---|---|---|---|---|---|---|
| `naca0012` | NACA 0012 (blunt TE) | 1e6 | 0 | 9 | — | 160, TE/LE 0.15 |
| `naca0012_trip` | NACA 0012 | 1e6 | 0 | 9 | x/c = 0.1 both sides | 160 |
| `naca0012_sharp` | NACA 0012, sharp TE | 1e6 | 0 | 9 | — | 160 |
| `naca0012_n240` | NACA 0012 | 1e6 | 0 | 9 | — | XFOIL 6.996 defaults (240, 0.3) |
| `naca2412` | NACA 2412 | 3e6 | 0 and 0.3 | 9 | — | 160 |
| `naca2412_ncrit` | NACA 2412 | 1e6 | 0 | 5 top, 11 bottom | — | 160 |
| `naca4415` | NACA 4415 | 1e6 | 0 | 9 | — | 160 |
| `naca23012` | NACA 23012 | 6e6 | 0 | 9 | — | 160 |
| `clarky` | Clark Y (UIUC) | 5e5 | 0 | 9 | — | 160 |
| `e387` | Eppler 387 (UIUC) | 2e5 | 0 | 9 | — | 160 |

## XFOIL sessions

Each `[[viscous]]` run of a manifest is computed in its own XFOIL session
(`xfoil_<name>.in`), so that it starts from XFOIL's initial state:

1. airfoil, paneling (`PPAR`), `OPER`, `VISC <Re>`, `MACH`, `VPAR` (`N` or
   `NT`/`NB`, `XTR`);
2. the polar: `PACC` then `ASEQ` from an uninitialized boundary layer. XFOIL
   uses the converged boundary layer of each point as the initial guess of the
   next one (25 Newton iterations per point, sequence stopped after 4
   successive unconverged points), so a sequence is reproduced by running the
   same sequence;
3. the single points: `INIT` (reset of the boundary layer) then `ALFA`
   (20 iterations) for each angle, followed by `CPWR` and `DUMP`.

Unconverged points are not added to the polar files by XFOIL.

## Files

| File | XFOIL command | Content |
|---|---|---|
| `vpolar_<name>.txt` | `PACC` + `ASEQ` | alpha, CL, CD, CDp, CM, Cpmin, transition x/c and index |
| `vdump_<name>.bin` | `PACC` dump file | per converged point: forces, then x, Cp, θ, δ*, Cf, Ctau^½ or N at every boundary-layer station, full double precision |
| `vpoints_<name>.txt`, `vpdump_<name>.bin` | `PACC` | the same for the single points |
| `vcp_<name>_a<alpha>.txt` | `CPWR` | x, viscous Cp at the panel nodes (5 decimals) |
| `vbl_<name>_a<alpha>.txt` | `DUMP` | s, x, y, Ue, δ*, θ, Cf, Hk, H*, P, m, K at the nodes, then the wake (5–6 decimals) |
| `xfoil_<name>.log` | — | console transcript (Newton iterations) |

The dump files are gfortran sequential unformatted files (4-byte record
markers, 8-byte reals, 4-byte integers). Header records: name, `'XFOIL   '`,
version; Mach, Re/10⁶, Ncrit top, Ncrit bottom; Mach and Re types; `0 0 0 N`;
the N airfoil nodes. Then, for each point: alpha, CL, CD, 0, CM, x/c of
transition top and bottom; the station counts of both sides and the
trailing-edge station indices; one record per side with (x, Cp, θ, δ*, Cf,
Ctau^½ or N) at every station from the stagnation point to the end of the
wake.

## Regenerating

```sh
python3 tests/reference/generate.py --xfoil /path/to/xfoil --root tests/viscous
```

or `gh workflow run xfoil-reference.yml -f root=tests/viscous`.
