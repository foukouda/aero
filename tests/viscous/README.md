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
| `e387_re6e4` | Eppler 387 (UIUC) | 6e4 | 0 | 9 | — | 160 |
| `s1223` | Selig S1223 (UIUC), high lift | 1e5 | 0 | 9 | — | 160 |
| `mh32_re5e4` | Hepperle MH 32 (UIUC) | 5e4 | 0 | 9 | — | 160 |

The last three, with `sd7037_cl` below, cover the Reynolds numbers of small
slow models (a 20 km/h model with a 15–20 cm chord flies at 5·10⁴–10⁵), where
long laminar separation bubbles dominate.

`sd7037_cl` is chaotic: during the warm-started sweep the Newton iterations
go through unstable phases where last-bit differences grow, so which points
converge depends on round-off. Perturbing our own Reynolds number by 10⁻¹⁵
changes the number of converged points of the polar (15 unperturbed; 12, 14
and 13 at +10⁻¹⁵, −10⁻¹⁵ and +10⁻¹⁴; 2 at +10⁻¹², where the sweep stops after
four failures); XFOIL converges 14, all of them among ours. The test accepts
it only because the console logs are identical to XFOIL's up to a round-off
divergence (line 390, an iteration still far from convergence), and
`chaotic_cases_are_sensitive_to_round_off` checks the sensitivity itself.
At such Reynolds numbers a single unconverged point of a polar says little
about the airfoil.

Cases of the other operating modes (160 nodes, Ncrit 9, free transition):

| Case | Airfoil | Mode | Conditions |
|---|---|---|---|
| `naca2412_cl` | NACA 2412 | prescribed CL (`CSEQ`, `CL`) | Re 1e6 |
| `e387_cl` | Eppler 387 | prescribed CL | Re 2e5 (laminar bubbles) |
| `sd7037_cl` | SD7037 | prescribed CL | Re 6e4 |
| `naca4415_cl` | NACA 4415 | prescribed CL | Re 1e6; CL = 0.8 is a limit cycle: the bottom transition jumps between x/c = 0.99 and the trailing edge at every iteration, and the point does not converge |
| `naca4415_type2` | NACA 4415 | angles and prescribed CL, `TYPE 2` | M √CL = 0.2, Re √CL = 1e6 |
| `naca23012_type3` | NACA 23012 | prescribed CL, `TYPE 3` | Re CL = 1e6 |
| `naca0012_damp2` | NACA 0012 | angles, `DAMP` (modified amplification rate) | Re 1e6 |
| `naca2412_flap` | NACA 2412 | angles, flap hinge at (0.75, 0): `FNEW`, `FMOM`, `HINC` | inviscid and Re 1e6 |

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

In prescribed-CL mode (`mode = "cl"`), `CSEQ` and `CL` replace `ASEQ` and
`ALFA`; `type` sets `TYPE` (Mach and Reynolds numbers varying with CL),
`damp2` enables `DAMP`, and a case-level `hinge` adds `FNEW`, `FMOM` and
`HINC` (hinge moments written in the polars).

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
