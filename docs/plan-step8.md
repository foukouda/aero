# Step 8 — the shape explorer

Requested on 2026-10-26: "an interface to explore aircraft shapes more or
less easily: first set the conditions of the aircraft I want — weight,
size, efficiency, speed, flight altitude, stall, lift… —, then generate
aircraft shapes and run quick simulations to know which is best, then take
the best and mutate them, and so on; all of this must improve with a
database that gets better the more real simulations of the objects I run
(with OpenFOAM). That is the final idea: a simplified exploration tool for
aircraft shapes."

The loop: requirements → generate → quick simulation → keep the best →
mutate → repeat; selected designs go to a high-fidelity simulation
(OpenFOAM), whose results are stored and correct the quick simulation, so
that the explorer becomes more accurate with every real simulation.

## What exists, what is missing

Exists: the flight (speed, altitude, mass), the aircraft as sections with
airfoils (step 7 P1–P2b), random shapes (P2c), the 3D view and editor, the
2D viscous solver checked against XFOIL and the E387 wind-tunnel data, the
airfoil catalogue, constraint handling by Deb's rules (`aero-optim`).

Missing: the quick 3D simulation itself — step 7 P3 (AVL's vortex lattice,
checked against AVL) and P4 (trim, stability, drag of the whole aircraft)
come first —, the requirements, the evolution loop, the database, the
OpenFOAM bridge and the learning.

## X1 — the requirements

A "brief" next to the flight: mass, cruise speed and altitude (as today);
limits — span, length, stall speed, minimum static margin (stability),
minimum glide ratio or maximum power, allowed configurations and airfoils —;
and the objective — best glide ratio at cruise, least power, slowest
stall, longest endurance. Each limit and the objective say in words what
they mean, as the cards of "My aircraft" do.

## X2 — the quick simulation

For one design, in well under a second: the vortex lattice (P3) for the
lift distribution, induced drag and moments; the profile drag of each
strip from the 2D viscous polar of its airfoil at its own Reynolds number;
fuselage skin friction; trim (incidence or elevator) at the cruise speed;
static margin; the margin to stall of every section. Designs run in
parallel on all cores. The 2D polars, the costly part, are computed once
per airfoil and Reynolds band and kept in the database.

## X3 — the evolution loop

- A design is the aircraft's parameters (configuration, sections, airfoils,
  positions, centre of gravity) within bounds from the requirements.
- Variation: small random changes of the numbers, another airfoil from the
  catalogue or the library, a section added or removed, crossing two
  parents of the same configuration; every child is checked (`validate`)
  and the constraints ranked by Deb's rules, as in `aero-optim`.
- Selection: NSGA-II (non-dominated sorting, crowding distance) on the two
  chosen objectives, with Deb's constraint domination; reproducible by
  seed.
- Interface: progress by generation, the best designs as the gallery
  (picture, key numbers), the population as a scatter plot of two chosen
  quantities with the front of best trade-offs, a click to look at a design
  in 3D, edit it, or start a new run from it ("mutate this one"), and its
  parents.

## X4 — the database

A local SQLite file (app data folder): every design (its parameters and a
fingerprint), every evaluation with its fidelity (quick or CFD), solver
version and flight, the cached 2D polars, the CFD runs (case, mesh, status,
residuals, forces), the explorer runs and generations. Nothing is computed
twice; results can be re-ranked for new requirements, exported as CSV or
JSON.

## X5 — OpenFOAM, the real simulation

- Case written from the aircraft: the surface mesh as STL (half model and
  symmetry plane), snappyHexMesh in a box around it, inlet from the flight,
  steady incompressible RANS (simpleFoam) with k-ω SST, and the Langtry–
  Menter transition model (kOmegaSSTLM) at the low Reynolds numbers of
  models, where fully turbulent RANS mispredicts drag; forces and
  coefficients by function object; coarse and medium meshes.
- Running on this PC (8 cores, 64 GB) under WSL2, which is not installed
  yet and must live on D: (7 GB free on C:), or exported to run elsewhere
  and imported back. A queue in the application: send designs, follow the
  residuals, read the results into the database.
- Validated before being trusted, like the XFOIL port: the E387 against the
  NASA Langley measurements already in `tests/wind_tunnel`, and a straight
  wing against AVL + XFOIL at a high Reynolds number; documented.

## X6 — learning from the real simulations

- For every design simulated in CFD, the differences CFD − quick (lift,
  drag, moment) are modelled from the design's features (Reynolds number,
  aspect ratio, sweep, taper, thickness, lift coefficient, configuration…)
  by Gaussian process regression, which also says how uncertain it is.
- The explorer ranks on the corrected values with their uncertainty, and
  proposes which designs to simulate next (promising and uncertain).
- Honest about its limits: it helps only after tens of CFD runs, never
  extrapolates silently, and its accuracy is shown (leave-one-out).

## Decisions (2026-10-26)

- OpenFOAM runs on this PC under WSL2 (installed on D:), launched from the
  application; cases can also be exported to another machine and their
  results imported.
- Two objectives: the explorer looks for the front of best trade-offs
  (NSGA-II with constraint domination) between two chosen criteria, such as
  glide ratio and stall speed; the user then picks along the front.
- Everything may change, configuration included: variation can add or
  remove a canard or a tail, turn an aircraft into a flying wing and back
  (the new parts sized by volume coefficient, as the random generator
  does), besides shapes and airfoils.

## Order and checks

Step 7 P3 → P4 → X1 → X2 → X3 (the explorer is usable from here, with quick
simulations) → X4 → X5 → X6. Same rules as before: tests green, injected
bugs caught, solvers checked against references, UI checked on screenshots,
texts in English and French.
