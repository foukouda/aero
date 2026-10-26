# Glossary

The quantities and terms of the Aero application, explained for non-specialists. The same text is shown in the application (Glossary button, and on the labels with a dotted underline).

_Generated from `app/src/lib/glossary.ts` by `npm run glossary`; do not edit by hand. Version française : [glossaire.md](glossaire.md)._

## Basics

### Airfoil

**The cross-section of a wing, a propeller blade or a sail, cut parallel to the flow.**

An airfoil is the 2D shape you see when you cut a wing parallel to the flow: a rounded leading edge at the front, a sharp (or nearly sharp) trailing edge at the back, an upper and a lower surface.

Aero studies this 2D section, as if the wing were infinitely long: the results are per unit span. A real wing adds 3D effects (wing-tip vortices, induced drag) that are not computed here.

### Chord (c)

**The length of the airfoil, from the leading edge to the trailing edge; positions are given as x/c.**

The chord c is the straight line from the leading edge to the trailing edge. Aero works with airfoils scaled to a unit chord, so a position x/c = 0.3 means 30 % of the chord from the leading edge, and a thickness t/c = 0.12 means 12 % of the chord.

### Angle of attack (α)

**The angle between the chord and the oncoming flow, in degrees.**

The angle of attack α is the angle between the chord line and the direction of the oncoming air. Increasing it increases the lift, almost linearly (about +0.11 in Cl per degree), until the flow separates from the upper surface: that is the stall, usually between 10° and 18°.

A cambered airfoil already lifts at α = 0; it reaches zero lift at a slightly negative angle (about −2° for a NACA 2412).

### Polar

**The curves of an airfoil's coefficients over a range of angles of attack or lift coefficients.**

A polar describes the airfoil over its whole operating range: Cl(α) (lift curve), Cl(Cd) (drag polar), Cm(α) and the transition points. It is computed by a sweep, one point after the other.

On the drag polar Cl(Cd), the further left a curve is at a given Cl, the less drag the airfoil makes there. Laminar airfoils show a “drag bucket”: a range of Cl where the drag is particularly low.

### Sweep

**A series of analyses at regularly spaced angles of attack (or lift coefficients), which builds a polar.**

A sweep computes the airfoil at α = start, start + step, … up to the end value (or the same with Cl). In viscous mode each point starts from the boundary layer of the previous one, as XFOIL's ASEQ and CSEQ commands do: it is faster and converges more often. Points that do not converge (typically beyond the stall) are left out of the polar.

### XFOIL

**The reference airfoil analysis program by Mark Drela (MIT), which Aero rewrites and is validated against.**

XFOIL (1986–2013) is the most widely used airfoil analysis program, written in Fortran by Mark Drela at MIT. It couples a panel method for the outer flow with an integral boundary-layer model, which gives lift, drag, moment and transition in a fraction of a second.

Aero is a rewrite of XFOIL 6.996 in Rust; its results are checked against those of XFOIL on reference cases, down to the digits XFOIL prints.

## Forces and coefficients

### Dimensionless coefficient

**A force divided by the dynamic pressure and the chord, so that results do not depend on size or speed.**

Forces grow with the air density ρ, the square of the speed V and the size. Dividing a force per unit span by the dynamic pressure q = ½ρV² and by the chord c gives a coefficient that depends only on the shape, the angle of attack and the Reynolds and Mach numbers. For example the lift per metre of span is L = q · c · Cl.

Example: at 20 m/s at sea level, q ≈ 245 Pa; a 0.2 m chord with Cl = 0.5 lifts 245 × 0.2 × 0.5 ≈ 25 N per metre of span.

### Lift coefficient (Cl)

**The lift (force perpendicular to the flow) in dimensionless form; higher means more lift.**

Cl measures the lift, the force perpendicular to the oncoming flow that holds an aircraft up. Typical values: 0 to 0.5 in cruise, 1.2 to 1.6 at the maximum for usual airfoils (Cl max, just before the stall), up to 2 or more for high-lift airfoils at low speed.

Cl grows almost linearly with the angle of attack (the theoretical slope is 2π per radian, about 0.11 per degree) until the stall, where it levels off and drops.

### Drag coefficient (Cd)

**The drag (force along the flow, resisting the motion) in dimensionless form; lower is better.**

Cd measures the drag, the force along the flow that the engine (or gravity, for a glider) must overcome. For a 2D airfoil it comes from friction on the surface and from the pressure difference created by the thickening boundary layer and the wake (see Cdp and Cdf).

Typical values: 0.004 to 0.008 for a good airfoil in its normal range, more beyond the stall. Engineers often speak of drag counts: 1 count = 0.0001, so Cd = 0.0060 is 60 counts. Cd is only meaningful in viscous mode: an inviscid computation gives no drag.

### Pressure and friction drag (Cdp / Cdf)

**The two parts of the drag: Cdf from skin friction, Cdp from the pressure field (form drag).**

Cdf is the friction of the air on the surface: it is low while the boundary layer is laminar and much higher once it is turbulent, which is why moving the transition back reduces drag. Cdp, the pressure (or form) drag, comes from the displacement of the flow by the boundary layer and grows strongly when the flow separates, near the stall.

At moderate angles, friction is usually the larger part.

### Lift-to-drag ratio (finesse) (L/D)

**Cl divided by Cd: how much lift the airfoil gives per unit of drag; higher is better.**

L/D = Cl / Cd is the efficiency of the airfoil. Good airfoils reach 100 to 150 in 2D (a whole aircraft, with its fuselage and 3D effects, much less: about 15 for an airliner, 40 to 60 for a sailplane). Its maximum, and the Cl where it occurs, is a key figure when choosing an airfoil for gliding or long range.

### Moment coefficient (about the quarter chord) (Cm)

**The pitching moment about the point at 25 % of the chord; negative means the airfoil tends to pitch nose down.**

Cm measures the torque that the pressure exerts on the airfoil around a point at a quarter of the chord (c/4). For cambered airfoils it is negative and nearly constant before the stall: −0.05 for a NACA 2412, −0.1 or less for strongly cambered airfoils. A symmetric airfoil has Cm ≈ 0.

It matters for stability and structure: a strongly negative Cm needs a larger tail (or twist) to trim the aircraft and twists the wing; flying wings need airfoils with Cm ≥ 0 (reflexed trailing edge).

### Pressure coefficient (Cp)

**The pressure on the surface compared with the free-stream pressure; negative means suction.**

Cp = (p − p∞) / q tells how the local pressure p compares with the pressure far away p∞. Cp = 1 at the stagnation point (where the flow stops, on the leading edge), Cp < 0 where the air is faster than the free stream (suction, mostly on the upper surface).

The area between the upper and lower curves is the lift. By convention the plot has −Cp upwards, so that the upper surface (suction) is on top. A sharp suction peak followed by a steep rise of pressure promotes separation; Cp min is the lowest value.

### Stall

**Beyond a certain angle of attack the flow separates from the upper surface: lift drops and drag soars.**

As the angle of attack grows, the pressure rise on the rear upper surface becomes too steep for the boundary layer, which separates. Lift stops increasing (Cl max) and drops, drag and moment change abruptly.

Stalled flows are unsteady and hard to compute: near and beyond Cl max, XFOIL's method (and Aero's) becomes approximate and often does not converge — such points are left out of the polars. Treat computed Cl max values as estimates.

### Hinge moment (Ch)

**The moment of the pressure on a flap (the part behind a hinge point) about that hinge.**

If the rear part of the airfoil is a control surface (flap, aileron, elevator) hinged at a point, the air pushes on it and creates a moment about the hinge; Ch is that moment divided by q·c². It tells how hard the actuator or the pilot must work. Here a positive Ch tends to raise the trailing edge of the flap (XFOIL's FMOM convention).

## Flow conditions

### Reynolds number (Re)

**Speed × chord / viscosity of the air: compares inertia with viscosity; it sets the boundary-layer behaviour.**

Re = V · c / ν, where ν ≈ 1.5·10⁻⁵ m²/s for air at sea level. It is the most important parameter of a viscous analysis: at low Re the boundary layer stays laminar longer but separates easily (laminar bubbles), and the drag is higher; at high Re the flow is more turbulent and the airfoil reaches a higher Cl max.

Orders of magnitude: model aircraft 50 000 – 300 000, sailplanes 1 – 3 million, light aircraft 3 – 6 million, airliners 20 – 50 million. Example: 20 m/s with a 0.2 m chord gives Re ≈ 270 000.

### Mach number (M)

**Speed divided by the speed of sound; above about 0.3 the compressibility of the air starts to matter.**

M = V / a, with a ≈ 340 m/s at sea level. Below M ≈ 0.3 the air behaves as incompressible and M = 0 is fine. Above, the Kármán–Tsien correction accounts for compressibility, which increases the suction peaks.

When the local flow becomes supersonic somewhere on the airfoil (Cp min below the critical value Cp*), shock waves appear, which this method cannot represent: Aero then reports that the critical Mach number is exceeded, unless “allow supercritical” is ticked (results then become unreliable).

### Viscous and inviscid analysis

**Inviscid: ideal flow without friction (lift and moment only). Viscous: adds the boundary layer, hence drag, transition and stall.**

The inviscid analysis (panel method) treats the air as a perfect fluid. It is instantaneous and gives a good Cl and Cm at moderate angles, but no drag and no stall.

The viscous analysis couples it with a model of the thin layer of air slowed down by friction near the surface (the boundary layer), as XFOIL does. It gives the drag, the transition points and a realistic lift curve up to the stall. It needs a Reynolds number and takes a few tens of milliseconds per point.

## Boundary layer and transition

### Boundary layer

**The thin layer of air near the surface, slowed down by friction; it causes the drag and the stall.**

Right on the surface the air sticks to the wall (zero speed); a few millimetres away it moves at the outer speed. This thin layer is the boundary layer. It starts laminar (smooth, orderly) near the leading edge, then becomes turbulent (chaotic, with more friction) at the transition point. It continues behind the airfoil as the wake.

Its plots in the Analysis tab: δ* (displacement thickness, how far the outer flow is pushed away), θ (momentum thickness, related to the drag), H = δ*/θ (shape factor: about 2.6 laminar, 1.4 turbulent; it rises toward separation, beyond about 3.5 laminar or 2.5 turbulent), Cf (skin friction), N (amplification of the disturbances, see Ncrit) and Ue (speed at the edge of the layer).

### Transition (Xtr)

**Where the boundary layer turns from laminar to turbulent, as a fraction of the chord (Xtr), on each surface.**

Small disturbances in the laminar boundary layer grow until it becomes turbulent. Xtr top and Xtr bot give that position on the upper and lower surfaces (1 means laminar up to the trailing edge). Since turbulent friction is several times higher, a transition further back means less drag: that is the principle of laminar airfoils.

The transition moves forward on the upper surface and backward on the lower surface when the angle of attack increases. It is predicted with the eᴺ method (see Ncrit) or forced by a trip.

### Critical amplification ratio (Ncrit)

**How disturbed the incoming air is: the transition happens when disturbances have grown by e^Ncrit. 9 = average wind tunnel.**

The eᴺ method follows the growth of the disturbances in the laminar layer; N is the logarithm of their amplification. Transition happens when N reaches Ncrit, which depends on how calm the surrounding air is: 9 is the usual value (average wind tunnel), 11 to 14 for a sailplane in calm air, 4 to 8 for a noisy environment, a rough or vibrating surface.

A lower Ncrit makes the transition happen earlier, hence more drag. Aero lets you set it separately on each surface.

### Trip (forced transition) (Trip x/c)

**Forces the transition at a given x/c, like a turbulator strip or a rough edge. 1 = free transition.**

A trip forces the boundary layer to become turbulent at a given position, whatever the eᴺ method predicts — like a zig-zag tape, a step or dirt (insects) on a real wing. The transition then happens at the trip or earlier. Use it to model rough surfaces or to compare with wind-tunnel tests made with trips; 1 (the default) means free transition.

### Laminar separation bubble

**At low Reynolds numbers the laminar layer separates, turns turbulent and reattaches, leaving a small bubble that adds drag.**

Below Re ≈ 500 000 the laminar boundary layer often separates before it has become turbulent; the separated layer then turns turbulent and usually reattaches, enclosing a bubble. It shows as a plateau in Cp and a bump in δ* and H, and adds drag. XFOIL's method is known to predict these bubbles well — one of the reasons for its popularity for model aircraft and small drones.

## Geometry

### NACA designation

**Digits that describe a family of airfoils: NACA 2412 = 2 % camber at 40 % chord, 12 % thick.**

The NACA four-digit airfoils (1930s) are defined by formulas: in NACA MPTT, M is the maximum camber in % of the chord, P its position in tenths of the chord and TT the maximum thickness in %. NACA 0012 is symmetric and 12 % thick; NACA 4415 has 4 % camber at 40 % and is 15 % thick.

Five-digit airfoils (e.g. 23012) have their maximum camber further forward and a small pitching moment: the first digit × 0.15 is the design Cl, the second gives the position of the maximum camber (in twentieths of the chord), the third says whether the mean line is reflexed (1) or not (0), and the last two give the thickness.

### Thickness (t/c)

**The largest distance between the upper and lower surfaces, as a fraction of the chord.**

A thicker airfoil is stronger and stiffer (the spar is taller, the wing lighter) and holds more fuel, and it usually stalls more gently; a thinner one has less drag at high speed. Typical values: 8 – 10 % for fast or thin wings, 12 – 15 % for most aircraft, 18 – 21 % at the root of large wings. Its position (x/c of the maximum) matters for laminar flow.

### Camber

**How curved the airfoil is: the largest distance between the chord and the mean line, as a fraction of the chord.**

The mean line runs halfway between the upper and lower surfaces; the camber is its largest distance to the chord. Camber gives lift at zero angle of attack and raises Cl max, at the cost of a more negative Cm. Symmetric airfoils (0 %) are used for tails and aerobatics; 2 – 4 % is common for wings, 5 – 8 % for low-speed high-lift airfoils.

### Leading-edge radius

**The radius of the circle that fits the rounded nose, as a fraction of the chord.**

A larger leading-edge radius softens the suction peak at high angles of attack and gives a gentler stall; a small radius (sharp nose) stalls abruptly from the leading edge. For a NACA 0012 it is about 1.6 % of the chord.

### Trailing-edge angle and thickness

**The angle between the surfaces at the trailing edge, and the gap between them if it is not sharp.**

A very small trailing-edge angle is hard to build and fragile; a blunt trailing edge (a small gap, often 0.1 – 0.5 % of the chord) is easier to manufacture and is modelled by XFOIL as a small panel across the gap.

### Flap deflection

**The rear part of the airfoil is rotated about a hinge, positive down: more lift and more nose-down moment.**

A flap (or an aileron, an elevator) is the part of the airfoil behind a hinge. Deflecting it down by a few degrees cambers the airfoil: the lift at a given angle increases (roughly +0.05 to +0.1 in Cl per degree for a 25 % chord flap), the zero-lift angle moves to more negative values, and Cm becomes more negative. Deflected up, it does the opposite (reflex).

The hinge is given by its x position and either its y or its height as a fraction of the local thickness (0 on the lower surface, 1 on the upper one). As XFOIL does, the surface where the flap moves in is cut and a circular arc closes the gap on the other side; the rounded hinge of a real flap and the flow through its slot are not modelled.

### Modifying an airfoil

**XFOIL's geometry tools: flap, thickness and camber, their high points, trailing-edge gap, leading-edge radius, blending.**

Each tool creates a new airfoil in the library and keeps the original, so both can be compared. Thickness and camber can be scaled or set (camber line and thickness are measured vertically, as XFOIL does); the high points slide the maximum thickness or camber along the chord; the trailing-edge gap opens or closes the trailing edge over a blending distance; the leading-edge radius is scaled near the nose; blending interpolates between two airfoils point by point (a fraction outside 0–1 extrapolates). They are ports of XFOIL's GDES commands (FLAP, TFAC, TSET, HIGH, TGAP, LERA) and of INTE, checked against XFOIL.

### Panels and paneling

**The airfoil contour is cut into straight segments (panels) on which the flow is solved; 160 is a good default.**

The panel method represents the surface by N nodes joined by straight panels. XFOIL's PANGEN routine places them along a spline of the contour, closer together where the curvature is high (leading edge) and at the trailing edge. More panels give more accurate results but take longer; 160 to 240 nodes are usual. Results should hardly change when you increase the count — if they do, refine.

## Analysis options

### Prescribed α or Cl

**Either you set the angle of attack and get Cl, or you set Cl and the solver finds the angle (XFOIL's CL command).**

An aircraft in flight needs a given lift (to balance its weight), not a given angle. Prescribing Cl compares airfoils fairly at the same lift: which one makes the least drag at Cl = 0.5? The solver then finds the angle of attack together with the boundary layer.

### Mach and Reynolds varying with Cl (M, Re)

**For a given aircraft, flying at a higher Cl means flying slower: M and Re then decrease (XFOIL TYPE 2 and 3).**

With “fixed”, Re and M are the same at every point. With “∝ 1/√Cl” (XFOIL TYPE 2), they vary as for an aircraft of fixed weight and wing: the speed, hence M and Re, is proportional to 1/√Cl, and the values you enter are those at Cl = 1. With “Re ∝ 1/Cl” (TYPE 3), Re varies as 1/Cl (fixed lift, varying chord). This gives realistic polars for a given aircraft.

### Amplification model (Ampl.)

**Which correlation of disturbance growth is used for the transition: XFOIL's original one or the modified one of its DAMP command.**

Both are envelope correlations of the eᴺ method. The original one is XFOIL's default and the one used for its validation; the modified one (DAMP) changes the growth rates of the disturbances in some boundary-layer states. Keep “original” unless you are comparing with results obtained with DAMP.

### Convergence

**The viscous solution is found by successive corrections; it has converged when the corrections become negligible.**

The boundary layer and the outer flow are solved together by Newton iterations (20 at most for one point, as in XFOIL). Near the stall, or when the transition hesitates between two positions, the iterations may not settle: the point is then reported as not converged and should not be trusted. Approaching it from a neighbouring point (a sweep) often helps.

## Shape optimization

### Shape optimization

**The computer reshapes the airfoil, trying many candidates, to improve an objective while respecting constraints.**

You choose operating points (Cl or α, each with a weight), an objective (least drag, or best L/D) and constraints (thickness, moment…). The optimizer then analyzes hundreds of slightly different shapes with the viscous solver and keeps improving the best one.

An airfoil optimized for one point is often worse elsewhere: give several points, or check the polars afterwards (“Compare polars”).

### CST parameterization (CST)

**A way to describe a smooth airfoil with a few numbers (weights of polynomials), which the optimizer changes.**

The Class/Shape Transformation (Kulfan) writes each surface as a round-nose, sharp-tail “class” shape multiplied by a smooth polynomial. With order 6, each surface has 7 weights: 14 numbers describe the whole airfoil. They are first fitted to your airfoil; the fit error is shown. Higher orders allow finer shapes but make the search longer.

### Objective

**What the optimizer tries to improve: the weighted mean drag, or the weighted mean L/D, over the operating points.**

With several operating points, each has a weight: a point with weight 2 counts twice as much. A typical choice for a sailplane is a low Cl (fast cruise) and a high Cl (thermalling), with weights reflecting the time spent in each.

### Constraints and feasible designs

**Conditions the result must satisfy (minimum thickness…); a design that violates one is “infeasible”.**

Without constraints, an optimizer reducing drag would make the airfoil paper-thin. Constraints keep it buildable: minimum thickness (overall or at a spar station), minimum Cm, minimum trailing-edge angle. A design that violates a constraint, or whose analysis does not converge, is infeasible; a feasible design always beats an infeasible one, so the result satisfies the constraints whenever possible.

### CMA-ES and Nelder–Mead

**The two search methods: CMA-ES explores widely with populations of candidates, Nelder–Mead refines the best one.**

CMA-ES (covariance matrix adaptation evolution strategy) draws populations of random candidates around the current best, and learns which directions pay off; each population is analyzed in parallel on all cores. Nelder–Mead then polishes the result by moving a small simplex of candidates. The seed fixes the random draws: the same seed gives exactly the same result.
