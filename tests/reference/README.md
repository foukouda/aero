# XFOIL reference data

This directory holds the reference results used to validate the inviscid
solver against XFOIL. Every sub-directory with a `case.toml` is one case.

| Case | Airfoil | Paneling | Mach |
|---|---|---|---|
| `naca0012` | NACA 0012 (blunt TE) | 160 nodes, TE/LE 0.15 | 0, 0.3 |
| `naca0012_sharp` | NACA 0012, sharp TE (`NACS`) | 160 nodes, TE/LE 0.15 | 0 |
| `naca0012_n240` | NACA 0012 | XFOIL 6.996 defaults: 240 nodes, TE/LE 0.3 | 0 |
| `naca2412` | NACA 2412 | 160 nodes, TE/LE 0.15 | 0, 0.3 |
| `naca4415` | NACA 4415 | 160 nodes, TE/LE 0.15 | 0 |
| `naca23012` | NACA 23012 | 160 nodes, TE/LE 0.15 | 0 |
| `clarky` | Clark Y (UIUC) | 160 nodes, TE/LE 0.15 | 0, 0.3 |

## Provenance

- XFOIL **6.996** (latest release), official sources from
  <https://web.mit.edu/drela/Public/web/xfoil/xfoil6.996.tgz>
  (SHA-256 `0feb71b5…3dff3f23`), built with gfortran in **double precision**
  exactly as in Drela's `bin/Makefile_gfortran` (`-fdefault-real-8`).
- The build and the runs are performed by the manually triggered GitHub
  workflow `.github/workflows/xfoil-reference.yml`; its artifact is reviewed
  and committed here. Outputs are never edited by hand.
- `clarky/clarky_uiuc_lednicer.dat` is the unmodified UIUC Airfoil
  Coordinates Database file (Lednicer format). XFOIL only reads Selig-ordered
  files, so `generate.py` converts it to `clarky/clarky.dat`, keeping every
  digit verbatim.

## Files of a case

| File | XFOIL command | Content |
|---|---|---|
| `case.toml` | — | manifest: airfoil, paneling parameters, runs |
| `xfoil.in` | — | command file generated from the manifest |
| `panels.dat` | `PSAV` | paneled airfoil nodes (`G15.7`) |
| `polar_M<mach>.txt` | `PACC` + `ASEQ` | alpha, CL, CD, CDp, CM, Cpmin (4 decimals) |
| `cp_M<mach>_a<alpha>.txt` | `CPWR` | x, Cp at each node (5 decimals) |
| `xfoil.log` | — | console transcript |

## Regenerating

```sh
python3 tests/reference/generate.py --scripts-only            # scripts only
python3 tests/reference/generate.py --xfoil /path/to/xfoil     # full run
```

or trigger the workflow: `gh workflow run xfoil-reference.yml`.
