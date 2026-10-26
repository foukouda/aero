# Step 7 — the aircraft in 3D

Requested on 2026-10-26: start the 3D part, with a 3D viewer on the main
page ("My aircraft") where a beginner builds a simplified aircraft.

Today the aircraft is a flight and a rectangular wing (span × chord); its
3D behaviour is a textbook formula (induced drag with e = 0.9). This step
gives it a shape — wing, tail, fin, fuselage — shown in 3D and edited live,
then a real 3D analysis of that shape: the vortex lattice method of AVL
(Drela and Youngren), ported and checked against AVL as XFOIL was, joined
to the 2D viscous polars for the drag. A beginner then learns whether the
aircraft flies, whether it is stable, where to put the centre of gravity
and which part of the wing stalls first.

## P1 — the parametric aircraft (`aero-aircraft` crate)

Pure data and geometry, SI units, tested by hand calculations.

- Lifting surfaces, mirrored about the plane of symmetry: a list of
  sections (root, optional break, tip), each with its chord, leading-edge
  position (sweep), height (dihedral), twist and airfoil. Simple inputs —
  span, root and tip chords, sweep, dihedral, incidence — generate them.
- A classic aircraft: wing, horizontal tail, vertical fin, fuselage (body
  of revolution: length, diameter, nose and tail shapes); a flying wing:
  no tail, reflexed airfoil, sweep and twist. V-tail and canard later.
- Mass and centre of gravity: the mass of the flight; the CG position is
  an input, with a helper that places it for a chosen stability margin
  once P4 exists.
- Derived quantities with a reading for beginners: area, aspect ratio,
  taper, mean aerodynamic chord (MAC) and its position, tail volume
  coefficients (Vh = Sh·lh / (S·c̄), Vv = Sv·lv / (S·b)) against the usual
  ranges ("the tail is small for a trainer").
- Surface mesh for the viewer: each surface lofted from the real airfoil
  coordinates (the recommended airfoil goes on the wing), the fuselage as a
  body of revolution.
- Presets: trainer, glider, flying wing, light aircraft (Cessna 172).

## P2 — the 3D viewer in "My aircraft"

- three.js r186 bundled with the application (no network), orbit, zoom and
  pan, views 3/4, top, front and side, a ground grid in metres and a person
  for scale.
- The aircraft updates as the inputs change: panels Wing, Tail, Fin,
  Fuselage, Mass & CG, next to the viewer. Clicking a part in 3D opens its
  panel. Markers: centre of gravity, MAC, later the neutral point.
- The flight cards and the recommendations use the 3D wing (its area and
  mean chord instead of span × chord); "use this airfoil" puts the chosen
  airfoil on the wing in 3D.
- Saved in the project (format 5; format 4 opens with a wing built from its
  span × chord).

## P2b — free shapes and canards (requested on 2026-10-26)

"Let me add canards, change the airfoil in the middle of the wing, and
change the height, dihedral and rotation of each section — for the tail
too — to make really the shape I want, kept simple."

- Every lifting surface (wing, canard, horizontal tail) and the fin is a
  list of sections, as in XFLR5 and AVL: position along the span, chord,
  leading-edge offset (sweep), height (dihedral), twist and airfoil of each
  section; the surface has a position and an incidence. Sections can be
  added (splitting a panel) and removed; a span field scales the whole
  surface. The straight-tapered description stays as the way presets and
  new parts are made.
- A canard: a mirrored lifting surface ahead of the wing, with its volume
  coefficient Sc·lc / (S·c̄) shown next to the tail's.
- Project format 6: format 5 surfaces (root and tip) become two sections.

## P3 — vortex lattice (AVL port, `aero-vlm` crate)

- AVL's method: horseshoe vortices on the camber surfaces (chordwise and
  spanwise spacing as AVL: cosine, sine, equal), the aerodynamic influence
  matrix, forces by Kutta–Joukowski, induced drag in the Trefftz plane,
  stability derivatives (CLα, Cmα, …) and the neutral point.
- References: AVL 3.52 (GPL, official sources) built in CI like XFOIL,
  sessions on a rectangular wing, a tapered swept wing with dihedral and
  twist, a wing with tail and a flying wing; plus the classical checks
  (elliptic loading, CLα of a high-aspect-ratio wing tending to 2π·AR /
  (AR + 2)). Injected-bug tests as for the other solvers.

## P4 — how the aircraft flies

- Trim: the angle of attack and the elevator (or tail incidence) that fly
  the mission's speed with the moment balanced.
- Stability: static margin as a sentence ("stable, 12 % margin" / "tail
  heavy: move the CG forward by 2 cm"), the neutral point in 3D.
- Lift distribution along the span, in 3D (coloured strips) and as a plot,
  compared with each section's maximum lift from its viscous polar: which
  part stalls first (the tips of a strongly tapered wing).
- Drag of the whole aircraft: induced (Trefftz plane) + profile drag of each
  strip at its own Reynolds number and lift (the viscous 2D polars, as
  XFLR5's viscous VLM) + fuselage skin friction; power and glide ratio.
  This replaces the simple wing estimate of step 6 when a 3D aircraft
  exists.

## P5 — documentation

Glossary (mean aerodynamic chord, neutral point, static margin, tail
volume, dihedral, sweep, taper, trim, lift distribution), guided tour of
the viewer, README, NOTICE (three.js, AVL references), release.

## Decisions (2026-10-26)

- The viewer is in the middle of "My aircraft": the form on the left, the
  3D view large in the centre, the cards and the recommendations below.
- First version: classic aircraft (wing, tail, fin, fuselage) and flying
  wing; V-tail and canard later.
- The 3D comes first; the rest of step 6 (U1–U4, D) follows.

## Status

- P1 done on 2026-10-26: crate `aero-aircraft` — parametric surfaces and
  fuselage, four example aircraft, planform properties checked against
  the trapezoid formulas, tail volumes in the usual ranges for the
  examples, a closed and consistently oriented mesh (a degenerate
  leading-edge triangle found by the closedness test and fixed); 20
  injected bugs all caught.
- P2 done on 2026-10-26: the 3D view in the middle of "My aircraft"
  (three.js 0.186.1, orbit, standard views, grid in round lengths, centre
  of gravity and mean chord markers, click a part to open its panel), the
  aircraft built part by part in the form (wing, tail, fin, fuselage,
  centre of gravity, airfoil per surface), "Start from" with four example
  aircraft and their typical flights, a "Tail sizes" card, "Put it on my
  wing" on the recommendations. The flight uses the aircraft's wing (area,
  mean geometric chord); "size the chords for me" keeps span and taper.
  Project format 5 (a frozen format-4 project opens with the trainer scaled
  to its wing).
- P2b done on 2026-10-26: wing, canard, tail and fin as lists of sections
  (position, chord, offset and sweep, height and dihedral, twist, airfoil
  of each), sections inserted, removed or added at the tip, the span field
  stretching the surface; a canard (placed behind the nose when there is
  room ahead of the wing) with its volume coefficient. Project format 6 (a
  frozen format-5 project opens with two sections per surface). 12 more
  injected bugs caught.
- P2c done on 2026-10-26 (requested: "a button that generates the number
  of aircraft I ask, with random airfoils and shapes, see them all neatly
  and click one to zoom"): `aero_aircraft::random` draws classic, canard,
  flying-wing and three-surface aircraft (2 to 4 wing sections, gull and
  polyhedral wings, airfoils by role from the catalogue), the canard, tail
  and fin sized on their real arm for usual volume coefficients (0.3–0.6,
  0.4–0.7, 0.025–0.045; smaller fins on canard aircraft); 2000 draws all
  valid, reproducible by seed, 5 injected bugs caught. In the application a
  gallery of up to 60 pictures rendered from the same angle with one WebGL
  context, a large 3D view on click (previous, next, use this aircraft).
- P3 done on 2026-10-26: crate `aero-vlm`, a port of AVL 3.52's vortex
  lattice — spacings, camber slopes from NACA digits or airfoil
  coordinates, strips and normals, mirror images and the `y = 0` symmetry
  plane, Prandtl–Glauert, vortex cores between components, `NOWAKE` and
  `NOLOAD` surfaces, linearized control deflections, Kutta–Joukowski forces
  by strip, surface and total, Trefftz-plane drag and span efficiency,
  stability and control derivatives, neutral point — and a reader of AVL
  geometry files. The lattice is factored once with the unit solutions;
  a flow condition then costs a time proportional to its size. Left out:
  bodies, ground effect, design variables, `CDCL` polars (the profile drag
  will come from the 2D viscous polars, P4). AVL 3.52 built in CI
  (`avl-reference` workflow) on five cases and nine runs (`tests/avl`):
  every printed total, surface and strip force and derivative agrees within
  10⁻¹² (mostly 10⁻¹⁵). Independent checks: elliptic wing (e = 1.00,
  elliptic loading, CLα within 2 % of Helmbold), rectangular wings tending
  to the lifting line as the aspect ratio grows, half model = whole model,
  every derivative = finite differences. One AVL quirk kept and documented:
  its alpha derivatives turn non-zero roll and yaw rates with the wrong sign
  (no effect in steady flight). 33 injected bugs all caught.

## Order and checks

P1 → P2 (the visible part: build the aircraft in 3D) → P3 → P4 → P5. Each
part: tests green, clippy, docs, frontend checks, commit; UI parts checked
by driving the application and reading screenshots. New interface texts in
English and French from the start (`lib/i18n.ts`).
