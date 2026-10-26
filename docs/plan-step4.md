# Step 4 — explanations, many airfoils at once, publication

Requested on 2026-10-26: explain the quantities (Cl, Cd, …) to users who
are not aerodynamicists, analyze many airfoils at once, and make the
repository public.

## S1 — glossary

- One bilingual (English/French) glossary, `app/src/lib/glossary.ts`: what
  each quantity means physically, its typical values and how to read it
  (airfoil, chord, α, Cl, Cd, Cdp/Cdf, L/D, Cm, Cp, Re, Mach, viscous vs
  inviscid, boundary layer quantities, transition, Ncrit, trips, stall,
  polars, paneling, NACA designations, geometry, hinge moment, prescribed
  Cl, TYPE 2/3, DAMP, optimization terms).
- In the application: a Glossary panel (search, language shared with the
  tour) and `Term` labels on the column headers and controls: hovering
  shows the short definition, clicking opens the entry.
- For GitHub readers: `docs/glossary.md` and `docs/glossaire.md`, generated
  from the same source (`npm run glossary`), with a test that fails when
  they are out of date.

## S2 — many airfoils at once

- NACA field accepting a list (`0012, 2412, 4415 23012`).
- Import of a whole folder of coordinate files (button, or dropping a
  folder on the window); a report of the files that could not be read.
- "Add examples": a curated set (NACA 0012, 2412, 4412, 4415, 23012, 6409,
  Clark Y, Eppler 387).
- Library: show all / hide all / delete hidden; more plot colors.

## S3 — sweeping them all

- Sweeps of several airfoils run in parallel (one airfoil per core), with
  the same results as one by one.
- Comparison table of the polars: Cl max (and its α), Cd min, L/D max (and
  its Cl), Cd and α at a chosen Cl, Cm at zero lift; sortable, CSV export.

## S4 — documentation and tour

README, guided tour steps for the batch tools, the comparison table and
the glossary.

## S5 — publication

Commits rewritten with the GitHub no-reply address (the content is
unchanged), force-pushed, old workflow runs deleted, repository made
public with a description and topics.
