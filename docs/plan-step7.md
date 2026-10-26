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

## Order and checks

P1 → P2 (the visible part: build the aircraft in 3D) → P3 → P4 → P5. Each
part: tests green, clippy, docs, frontend checks, commit; UI parts checked
by driving the application and reading screenshots. New interface texts in
English and French from the start (`lib/i18n.ts`).
