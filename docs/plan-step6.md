# Step 6 — from the aircraft to the airfoil (a beginner's way in)

Requested on 2026-10-26: make Aero as easy as possible to pick up for
someone who is starting aerodynamics, so that they can start from a simple
hypothesis — "I want a plane that flies at 20 km/h, 100 m high, that
weighs this much" — instead of starting from an airfoil designation.

Today every path starts from an airfoil (NACA 2412, a `.dat` file) and from
solver inputs (Re, Ncrit, trip, Mach). A beginner does not know which
airfoil to pick, nor what Reynolds number their plane flies at. The new
entry point turns the flight they have in mind into those inputs, explains
what the numbers mean, recommends airfoils, and hands the case over to the
existing tabs, which stay as they are for experienced users.

## What the audit found

The current app was driven and captured on every tab (eight example
airfoils, analysis at α = 4°, viscous sweep −6…12°).

| Finding | Severity | Direction |
|---|---|---|
| The first screen is an empty plot and a 24-step tour; the only first action offered is "type a NACA designation" | critical | A start screen: "start from my aircraft", "study an airfoil", "open a project" |
| Nothing links the solver inputs to a real flight: Re = 1e6 means nothing to a beginner, speed, chord and altitude are nowhere | critical | Mission inputs in everyday units; Re, Mach and Cl derived and explained |
| Expert settings shown at the same level as the essentials: Ncrit (two fields), Trip x/c (two fields), M.Re, Ampl., Cdp / Cdf, Xtr, boundary-layer plots | moderate | Defaults in words ("calm air", "wind tunnel"), the rest behind "Advanced" |
| Legends explode with several airfoils: 24 entries under the Cp plot (upper / lower / inviscid × 8), half the panel height | moderate | One entry per airfoil, line styles explained once; highlight the selected airfoil and grey the others |
| The series palette fails the colour-vision check: green #16a34a next to red #dc2626, ΔE 5.0 for deuteranopes (the `dataviz` validator asks for ≥ 8); 16 colours cycled | moderate | A validated 8-colour order, dashes beyond 8, colour follows the airfoil |
| Results are numbers only (Cl 0.7146, L/D 103.1) with no reading | moderate | One sentence per result: "at 4°, NACA 2412 makes 103 times more lift than drag" |
| Number format mixes locales: inputs show "0,75" (Windows locale), outputs "0.300" | minor | One format everywhere |
| Base font 13 px, many 11–12 px labels | minor | 14 px base, a clearer type scale |
| The "Some points failed" toast does not say which airfoils nor what to do | minor | Name them, link to the explanation (stall, laminar bubble) |

Side finding, to check separately: in the sweep at Re = 10⁶, the NACA 6409
polar stops near α = 1°, while the single analysis at α = 4° converges.

## Examples that inspired the plan

- NASA FoilSim: speed, altitude, angle, thickness, camber and wing area as
  sliders, lift updated at once, in a classroom exercise "design a wing that
  makes this much lift". The model for the reactive mission sheet.
- PhET simulations (University of Colorado): "implicit scaffolding" —
  well-chosen controls, constraints and immediate feedback guide better than
  written instructions. The model for limiting what is shown at first.
- eCalc: mass and wing area give wing loading and stall speed in units an
  RC modeller knows (g/dm², km/h), with the flight envelope as a green band.
- XFLR5: the reference for wings from XFOIL polars, but it opens on an empty
  window and buries the workflow in menus (airfoil → wing → analysis). What
  to avoid.
- Bret Victor's "explorable explanations": a text whose numbers change when
  a parameter is dragged. The model for the "what this means" sentences.
- Progressive disclosure (Nielsen): beginners see little and make fewer
  mistakes, experts pay one click for the rest.

## M1 — mission engine (`aero_mission` crate)

Pure functions, f64, serde, no global state, unit-tested against hand
calculations and published tables.

- ISA atmosphere 0–11 km (with an optional temperature offset): T, p, ρ,
  μ (Sutherland), ν, speed of sound. Checked against the ISA tables at 0,
  1 000, 5 000 and 11 000 m.
- Flight conditions: dynamic pressure q = ½ρV², Re = Vc/ν, Mach = V/a.
- Level flight: lift = weight (load factor 1), required lift coefficient
  CL = mg / (qS), wing loading (g/dm² and N/m²).
- Wing estimate, clearly labelled as an estimate: aspect ratio, induced drag
  CDi = CL² / (π e AR) with e = 0.9 by default, profile drag from the airfoil
  polar, wing drag and power D·V, glide ratio and sink rate, stall speed from
  the airfoil's Cl max (× 0.9 for the 3D wing). A real wing analysis (lifting
  line, vortex lattice) remains the 3D step proposed earlier.
- Sizing mode: from speed and mass, the wing area that flies at a
  comfortable Cl (not above 80 % of Cl max); Re depends on the chord, so the
  area and the chord are found by a short fixed-point iteration.

Example (the hypothesis of the request): 20 km/h at 100 m gives
ρ = 1.213 kg/m³, q = 18.7 Pa, Mach 0.016. A 300 g model with a 1 m × 15 cm
wing needs CL ≈ 1.05 at Re ≈ 57 000 — close to stall for most airfoils;
with 1.2 m × 20 cm, CL ≈ 0.65 at Re ≈ 75 000. This is exactly what the
screen must make visible.

## M2 — an airfoil catalogue sorted by use

About 25 well-known airfoils with a short "good for" text: slow flyers and
high lift (S1223, E423…), gliders (SD7037, MH32, RG15…), trainers (Clark Y,
NACA 2412 / 4412), aerobatics (NACA 0012…), flying wings with reflex (MH45,
E186…). Coordinates from the UIUC database; the licence of each file and
the redistribution terms must be checked before bundling (NOTICE updated).

Low Reynolds numbers: the viscous references stop at Re = 10⁵ today, while
a 20 km/h model flies at 5·10⁴ – 10⁵. New XFOIL references at Re = 5·10⁴ and
10⁵ for E387, SD7037 and S1223 (xfoil-reference workflow), and a comparison
of E387 with the UIUC wind-tunnel measurements (Selig et al.), reported as a
physics check, not as XFOIL equivalence. Below 10⁵ the app shows that the
results are less reliable (laminar separation bubbles).

## M3 — recommending airfoils for a mission

For each catalogue airfoil (and the user's own): a viscous sweep at the
mission Re from 0 outward (existing parallel sweeps), Cl max, Cd at the
required Cl, stall margin. Ranking by what the user wants, in plain words:

| Goal | Criterion |
|---|---|
| Fly slowly / carry a load | highest Cl max (largest stall margin) |
| Least drag at my speed (glide far, fly longest) | lowest Cd at the required Cl |
| Easy to build | thickness ≥ 8 %, flat-ish lower surface |

For a given flight (speed, mass and wing fixed) the required Cl is fixed, so
"glide far" (best L/D) and "stay up longest" (best Cl^1.5/Cd) rank the
airfoils the same way: both come down to the lowest drag at that Cl. They
are one goal.

Airfoils whose required Cl is above 80 % of their Cl max, or that do not
converge there, are listed as "not suited" with the reason. Tested on cases
whose ranking is known (e.g. S1223 first for slow flight at Re 2·10⁵).

## M4 — the "My aircraft" tab

The first tab, and the start screen's recommended path.

- Inputs as sentences, everyday units, presets: "I want to fly at [20] km/h,
  [100] m high, with a model of [300] g. My wing is [1.0] m wide and [15] cm
  deep" — or "find the wing size for me". Presets: park flyer, 1.5 m glider,
  foam trainer, 2 kg drone at 60 km/h, a light aircraft for comparison.
- "What this means" cards, updated as you type: Re, required Cl, wing
  loading, stall speed, Mach; each with a status (ok / tight / too high —
  icon and word, not colour alone), one sentence ("Cl 1.05 is high: most
  airfoils are close to stall; make the wing larger or fly faster") and a
  glossary link. A to-scale top view of the wing next to a familiar object
  (an A4 sheet, a person) for size.
- Recommendations: the three best airfoils as cards (shape, key numbers,
  why), the full ranking as a table, and a polar where the three are
  highlighted and the others grey.
- Wing estimate: drag, power, glide ratio, stall speed, marked "wing only,
  simple formula".
- Hand-over: "use this airfoil" adds it to the library and sets Re, Mach and
  Cl in Analysis, Polars and Optimize; "optimize for my mission" opens
  Optimize with the required Cl as operating point and a stall-margin
  constraint. The mission is saved in the project (format 4, older files
  still open).

## U1 — start screen, simple and advanced settings, short tours

- Start screen instead of an empty plot: three choices and the presets.
- Simple / advanced: the analysis panels show speed or Re, angle or Cl and
  "viscous"; Ncrit (as "air turbulence: calm / normal / wind tunnel"),
  trips, Mach–Re mode, amplification model and boundary-layer plots go
  under "Advanced". Nothing is removed.
- The 24-step tour becomes a short tour per tab (3–5 steps), the mission
  flow being the guide for beginners.

## U2 — charts

- Palette validated with the `dataviz` skill script (light and dark), 8
  colours in a fixed order, dash patterns beyond, colour tied to the
  airfoil.
- Legends: one entry per airfoil, upper / lower / inviscid explained once
  in the title; the selected airfoil emphasised, the others grey on demand.
- Hover tooltips with the values; axis titles in words ("Lift coefficient
  Cl", "Angle of attack α (°)").

## U3 — words, units, type

- One-sentence readings of results (analysis table, polar summary, toasts
  that name the airfoils concerned and say what to try).
- One number format, units on every field, 14 px base and a clearer scale.
- Copy reviewed with the `design:ux-copy` skill, contrast and keyboard with
  `design:accessibility-review`; each UI step checked on screenshots with
  `design:design-critique`.

## U4 — French interface

Decided on 2026-10-26: the whole interface gets an EN / FR switch (it was
English only, the tour and the glossary already bilingual). Strings move to
one dictionary per language, checked by a test that both have the same
keys; the language follows the tour's language setting. Code, docs and
commits stay in English.

## D — documentation

Glossary entries (dynamic pressure, standard atmosphere, wing loading,
aspect ratio, induced drag, stall speed, wing area), README, release 0.3.0.

## Order and checks

## Status

- M1 done on 2026-10-26: crate `aero-mission` (atmosphere checked against
  the 1976 tables, conditions, wing estimates, sizing; 16 injected bugs all
  caught).
- M2 done on 2026-10-26: catalogue of 20 airfoils in five categories (UIUC
  coordinates bundled with attribution, decided with the user), tests that
  the descriptions match the shapes (symmetry, thickness, camber, reflex
  pitching moment). Four XFOIL references at Re = 5·10⁴ – 10⁵ (`e387_re6e4`,
  `mh32_re5e4`, `s1223`, `sd7037_cl`); the SD7037 case turned out chaotic
  (perturbing Re by 10⁻¹⁵ changes which points converge), accepted
  explicitly and documented. E387 compared with the NASA Langley
  measurements at Re = 10⁵ and 2·10⁵ (`tests/wind_tunnel`): Cl within 0.07,
  Cd within 25 % before stall, Cl max over-predicted by about 10 %.
- M3 done on 2026-10-26: `aero_mission::recommend` (polar from 0° up to
  the stall and down only as far as needed, Cl max, interpolated operating
  point, fit, buildability, ranking for three goals; 13 injected bugs all
  caught) and the application commands (conditions, wing sizing, parallel
  evaluation of catalogue and library airfoils with progress and
  cancellation). At Re ≈ 10⁵ and Cl 0.56 the glider sections come first
  (Cd ≈ 0.015) and the S1223 has three times their drag; at Cl 1.37 only
  the S1223 can fly.

Validated on 2026-10-26: the mission parts first, then the interface.
M1 → M2 → M3 → M4 → U1 → U2 → U3 → U4 → D. Each part: tests green,
`cargo clippy -- -D warnings`, `npm run check` and the frontend tests,
`RUSTDOCFLAGS=-D warnings cargo doc`, then a commit. UI parts are checked by
driving the app and reading screenshots.
