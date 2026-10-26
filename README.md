# aero

A modern rewrite of [XFOIL](https://web.mit.edu/drela/Public/web/xfoil/)
(Mark Drela) in Rust, with a Tauri desktop application. The goal is a robust,
parallel-friendly airfoil analysis and optimization toolkit that is usable as
a library.

New to airfoils? The [glossary](docs/glossary.md) ([en français](docs/glossaire.md))
explains every quantity — Cl, Cd, L/D, Cm, Cp, Reynolds number, transition… —
with orders of magnitude; the application shows the same explanations.

> **Status:** step 3 — geometry, XFOIL-compatible inviscid and viscous
> solvers (boundary layer, transition, drag, stall; prescribed angle or lift,
> Mach and Reynolds numbers varying with the lift, flap hinge moments), shape
> optimization and desktop application. 3D methods, Python bindings and
> WebAssembly come later; the architecture is prepared for them.

## Workspace layout

```
crates/
  geometry/       aero-geometry: airfoils, splines, .dat I/O, NACA/CST, PANGEN paneling
  foil2d/         aero-foil2d:   2D solvers (XFOIL panel method and viscous-inviscid interaction)
  optim/          aero-optim:    shape optimization (CST design space, CMA-ES, Nelder-Mead)
app/
  src-tauri/      aero-app: thin Tauri layer (commands -> Tauri-free engine -> crates)
  src/            Svelte 5 + TypeScript frontend (uPlot charts)
tests/reference/  XFOIL 6.996 inviscid reference results (tests/validation: random set)
tests/viscous/    XFOIL 6.996 viscous reference results (tests/viscous_validation: random set)
docs/             specifications and design notes
```

The computational crates never depend on Tauri, hold no global state, use
`f64` everywhere and return errors instead of panicking. Their public types
are serializable (`serde`) and, with the `specta` feature, exported to
TypeScript.

## Requirements

- Rust: the pinned toolchain (`rust-toolchain.toml`) is installed
  automatically by `rustup`.
- Node.js 20 or later (for the desktop application).
- Tauri system dependencies:
  - **Windows**: Microsoft C++ Build Tools and WebView2 (preinstalled on
    Windows 10/11).
  - **macOS**: Xcode Command Line Tools.
  - **Linux** (Debian/Ubuntu): `sudo apt install libwebkit2gtk-4.1-dev
    libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev
    build-essential`.

## Running the desktop application

```sh
cd app
npm install
npm run tauri dev      # development build with hot reload
npm run tauri build    # optimized installer in target/release/bundle
```

Features: airfoil library (NACA 4/5-digit generator with open or closed
trailing edge, `.dat` import in Selig or Lednicer format by button or drag
and drop, duplicate, rename, delete), true-scale shape view with optional
panel nodes and overlays, geometric properties, XFOIL `PANGEN` paneling
controls with immediate preview, single-point analysis driven by a live slider,
at a prescribed angle of attack or lift coefficient (Cp with −Cp up, Cl, Cm,
Cp min, optional Mach number with the Kármán–Tsien correction and a clear
error beyond the critical Mach number, optional flap hinge moment),
viscous analysis as in XFOIL's `VISC` mode (Reynolds number, Ncrit and
trips per surface, Mach and Reynolds numbers fixed or varying with the lift
as XFOIL's `TYPE 2` and `3`, original or modified amplification rate as
`DAMP`; Cl, Cd, L/D, Cm, transition points, pressure and friction drag;
viscous and inviscid Cp; boundary-layer plots of δ*, θ, H, Cf, N and Ue on
both surfaces and the wake), sweeps of the angle of attack or of the lift
coefficient with progress and cancellation (Cl(α), drag polar Cl(Cd), Cm(α),
hinge moments and transition points for several airfoils; viscous sweeps
warm-start each point from the previous one, as XFOIL's `ASEQ` and `CSEQ`,
starting near zero and marching outward), many airfoils at once (a list of
NACA designations, a whole folder of coordinate files, a set of classic
examples, show all / hide all) swept in parallel on all cores and compared
in a sortable table (zero-lift angle, Cl max, Cd min, L/D max, values at a
chosen Cl), shape optimization (see below) with live progress, a bilingual
glossary (hover or click the dotted labels), exports (`.dat`, CSV of polars, Cp and boundary layer,
PNG, SVG), JSON project files with a format version (older projects are
migrated), light and dark themes, and a guided tour in English and French.

## Libraries and tests

```sh
cargo test --workspace                                 # all Rust tests
cargo clippy --workspace --all-targets -- -D warnings
cargo bench -p aero-foil2d -p aero-optim               # criterion benchmarks
cd app && npm run check && npm test                    # frontend type check and tests
```

TypeScript bindings (`app/src/lib/bindings.ts`) are generated from the Rust
types by `tauri-specta`; a test fails when they are out of date. Regenerate
them with `npm run bindings` in `app/`.

```rust
use aero_foil2d::inviscid::{AlphaSweep, InviscidOptions, InviscidSolver, OperatingPoint};
use aero_foil2d::viscous::{ViscousOptions, ViscousSolver};
use aero_geometry::{NacaOptions, PanelingOptions, naca};

let airfoil = naca("2412", &NacaOptions::default())?.paneled(&PanelingOptions::default())?;
let solver = InviscidSolver::new(&airfoil, &InviscidOptions::default())?; // factorized once
let solution = solver.solve(OperatingPoint::incompressible(4.0))?;        // O(N) per alpha
println!("Cl = {:.4}, Cm = {:.4}", solution.cl, solution.cm);

// Viscous analysis (XFOIL VISC): boundary layer, transition, drag.
let options = ViscousOptions { reynolds: 1e6, ncrit: [9.0, 9.0], ..ViscousOptions::default() };
let viscous = ViscousSolver::new(&airfoil, &options)?;
let s = viscous.solve(4.0)?; // fresh boundary layer (XFOIL INIT + ALFA)
println!("Cl = {:.4}, Cd = {:.5}, x_tr = {:.3} / {:.3}", s.cl, s.cd, s.transition[0].x, s.transition[1].x);
let polar = viscous.sweep(&AlphaSweep { start_deg: -4.0, end_deg: 14.0, step_deg: 0.5 })?; // as ASEQ
```

`ViscousSolver::session()` gives full control over the warm starts (XFOIL's
`ALFA` without `INIT`) and can record XFOIL's console messages (iteration
history, transition points) for diagnosis. `solve_cl` and `sweep_cl`
prescribe the lift instead of the angle (XFOIL `CL`, `CSEQ`).

```rust
use std::ops::ControlFlow;
use aero_geometry::{NacaOptions, naca};
use aero_optim::{Constraints, DesignPoint, OptimizeOptions, Problem, Target, optimize};

// Minimum drag at CL = 0.5 and Re = 1e6 (the default viscous options),
// keeping the thickness at least 12 %.
let problem = Problem {
    points: vec![DesignPoint { target: Target::Cl(0.5), weight: 1.0 }],
    objective: Default::default(), // weighted mean Cd
    constraints: Constraints { min_thickness: Some(0.12), ..Default::default() },
    viscous: Default::default(),
    paneling: Default::default(),
    design: Default::default(), // CST of order 6: 14 variables
};
let options = OptimizeOptions { max_evaluations: 300, ..Default::default() };
let result = optimize(&naca("2412", &NacaOptions::default())?, &problem, &options, |progress| {
    println!("{} evaluations, best Cd {:?}", progress.evaluations, progress.best.objective);
    ControlFlow::Continue(()) // Break cancels and keeps the best design
})?;
let optimized = result.airfoil; // regular Airfoil: analyze it, export it as .dat
```

## Validation

The inviscid solver is checked against **XFOIL 6.996** (built from the
official sources, double precision) on seven cases — NACA 0012 (blunt TE,
sharp TE, 240-node paneling), NACA 2412, 4415, 23012 and the UIUC Clark Y —
at Mach 0 and 0.3 (`tests/reference`, `crates/foil2d/tests/xfoil_reference.rs`):

| Quantity | Our pipeline vs XFOIL | XFOIL print precision |
|---|---|---|
| Panel nodes (`PANGEN`) | ≤ 7·10⁻⁸ | 7 significant digits |
| Cl, Cm | ≤ 5·10⁻⁵ | 4 decimals |
| Cp at every node | ≤ 5·10⁻⁶ | 5 decimals |

i.e. the results agree to the precision XFOIL prints them. Three more
reference cases cover contours with corners (doubled points); on one of them
XFOIL itself produces NaN nodes, and aero reports an error instead.

An **extended validation** (`tests/validation`, `crates/foil2d/tests/xfoil_validation.rs`)
compares 100 randomly drawn cases with XFOIL — 60 airfoils of the UIUC
database and 40 NACA sections, with random paneling (60 to 360 nodes,
bunching, refinement zones) and Mach numbers — over 1292 polar points plus
the Cp distributions: all agree within the same bounds, and the importer
reads the 60 original UIUC files exactly like an independent Python
conversion. Mutation testing (deliberately injected bugs in the solver,
paneling, NACA generator and splines) confirmed that each injected bug
makes these tests fail. Theoretical
checks (`crates/foil2d/tests/theory.rs`): zero lift of symmetric sections at
zero incidence, lift slope tending to 2π for thin sections, thin-airfoil zero
lift angle, exact solutions for the circle, the ellipse and a Kármán–Trefftz
airfoil (lift within 10⁻⁴, second-order convergence of Cp), and convergence
of the lift with the number of panels.

### Viscous analysis

The viscous solver is a line-by-line port of XFOIL 6.996's
viscous–inviscid interaction (`xblsys.f`, `xbl.f`, `xsolve.f`, the wake and
coupling routines of `xpanel.f`, `VISCAL`): two-equation integral boundary
layer with Drela's closures, eᴺ transition (free or tripped), lag-dissipation
turbulence model, wake, source influence matrix and XFOIL's global Newton
system and block solver, with the same relaxations, limits and sequence
semantics, including the prescribed-lift mode (`CL`, `CSEQ`: the angle of
attack is a Newton variable), the Mach and Reynolds numbers varying with the
lift (`TYPE 2`, `3`), the modified amplification rate (`DAMP`) and the flap
hinge moments (`FNEW`, `FMOM`). It is checked against XFOIL 6.996 on 17
reference cases (`tests/viscous`, `crates/foil2d/tests/xfoil_viscous.rs`):
NACA 0012 (blunt and sharp TE, trips, 240 nodes, `DAMP`), NACA 2412 (Mach
0.3, different Ncrit on each side, prescribed CL, flap hinge), 4415
(prescribed CL, `TYPE 2`), 23012 (`TYPE 3`), Clark Y and Eppler 387 (laminar
separation bubbles, also at prescribed CL), at Reynolds numbers from 2·10⁵ to
6·10⁶, as polars (warm-started sequences) and single points:

- the same points converge and fail to converge as in XFOIL (253 converged
  points). One case is a point XFOIL cannot converge: NACA 4415 at Re = 10⁶
  and CL = 0.8, where the bottom transition jumps between x/c = 0.99 and the
  trailing edge at every Newton iteration; ours reproduces the same limit
  cycle iteration by iteration;
- forces, transition points and x, Cp, θ, δ*, Cf and N or √Cτ at every
  boundary-layer station agree with XFOIL's full-precision dump files to
  10⁻¹⁰ on most points (10⁻⁵ at worst, on a point that needs all its Newton
  iterations);
- XFOIL's console output — the rms change of every Newton iteration, the
  transition points, the marching failures — is reproduced line by line: 14
  of the 19 sessions are identical over their whole length (thousands of
  lines), the five others until a last printed digit late in a sequence;
- the hinge moments agree with XFOIL's to its printed precision.

An **extended viscous validation** (`tests/viscous_validation`,
`crates/foil2d/tests/xfoil_viscous_validation.rs`, [report](tests/viscous_validation/REPORT.md))
runs 50 random analyses (UIUC and NACA airfoils, Re from 10⁵ to 10⁷, Ncrit
per side, trips, Mach up to 0.4). XFOIL hangs on one of them, ours
terminates. 34 of the 49 others agree with XFOIL on every converged point. In
the 15 remaining ones the iterations go through unstable phases (separated
flows, points that do not converge), where last-bit differences between two
implementations (math libraries, operation order) are amplified until the
codes follow different paths: for each of them our console log is identical
to XFOIL's up to a first difference that is a last printed digit or no larger
than the change of that line when our own Reynolds number is perturbed by
10⁻¹⁵ to 10⁻¹¹. This log comparison found a real porting error (the handling
of a NaN residual), now fixed. Mutation testing — 18 deliberately injected
bugs: closure and transition constants, marching parameters, relaxation
limits, a sign in the Newton system, the block-solver threshold, wake
geometry and sources, the drag formula, the NaN handling — confirmed that
every one of them makes the validation fail (two are caught only by the
random set).

## Shape optimization

`aero-optim` reshapes an airfoil for a set of operating points. The shape is
described by CST (class/shape transformation) functions — Bernstein
polynomials of order 6 by default, 14 weights — fitted to the initial
airfoil by least squares; the trailing-edge thickness is kept. Each
operating point prescribes CL or α and has a weight; the objective is the
weighted mean of Cd, or of L/D. Constraints: minimum and maximum thickness,
minimum thickness at chord stations (spar), minimum Cm, minimum
trailing-edge angle.

Every candidate is analyzed by the same viscous solver, from a fresh
boundary layer at each point (if a point fails, it is approached from zero
lift in four warm-started steps, as one does with XFOIL). A point that still
does not converge — about 2 % of the candidates, mostly the transition limit
cycle described above — makes the candidate infeasible: it is never given a
made-up drag. Candidates are compared with Deb's feasibility rules (a
feasible design beats an infeasible one; infeasible designs compare by their
constraint violation), so the result satisfies the constraints whenever a
feasible design was found. The search is a CMA-ES (Hansen's default
parameters; each generation evaluated in parallel) followed by a Nelder–Mead
polish (Gao–Han coefficients). Runs are reproducible: the samples come from
a seeded generator and parallel results are collected in order, so the
result does not depend on the number of threads.

Checks (`crates/optim`): the optimizers on the sphere, Rosenbrock, an
anisotropic quadratic and a problem whose optimum lies on a constraint; the
evaluation against a hand-made analysis of the same airfoil; and end to end,
NACA 2412 at CL = 0.5, Re = 10⁶, t/c ≥ 12 %: 300 evaluations reduce Cd by
20 % (0.00592 → 0.00475; the upper-surface transition moves from x/c = 0.51
to 0.76). The optimized airfoil is then verified independently — written as
a `.dat` file, read back, analyzed through a CSEQ sweep (another solution
path) and with 240 panels instead of 160: the gain is unchanged (0.005946 →
0.004756), so it is not an artifact of the discretization. As always with
single-point optimization, the drag rises elsewhere on the polar; the
application's "Compare polars" button shows it.

## Performance

`cargo bench -p aero-foil2d` (NACA 2412, release build, one core of a
desktop CPU):

| Operation | Time |
|---|---|
| Assembly + LU factorization, 160 nodes | 1.7 ms |
| Assembly + LU factorization, 320 nodes | 7.5 ms |
| One angle of attack (Cp, Cl, Cm, Cp min) | 2.6 µs |
| Sweep of 101 angles of attack | 0.27 ms |
| `PANGEN` repaneling, 160 nodes | 0.23 ms |
| Viscous point from a fresh boundary layer, 160 nodes | 20 ms |
| Viscous sweep, 33 warm-started points | 0.41 s |

`cargo bench -p aero-optim` (same machine, 16 logical cores):

| Operation | Time |
|---|---|
| Evaluation of one design (geometry, paneling, one viscous point) | 27 ms |
| Optimization, 100 evaluations, one core | 3.3 s |
| Optimization, 100 evaluations, all cores | 1.5 s |

The CMA-ES generations run in parallel; the Nelder–Mead polish (a fifth of
the budget by default) is sequential by nature, which bounds the speed-up.

### Compared with XFOIL

The `speed-comparison` workflow builds XFOIL 6.996 from the official sources
and times the same computations as Aero on the same GitHub runner (AMD EPYC,
4 cores; [full report](tests/speed/REPORT.md), [method](tests/speed/README.md)):

| NACA 2412, 160 nodes, Re = 10⁶ | XFOIL, official build | XFOIL, `-O3` | Aero |
|---|---|---|---|
| Inviscid polar, 37 angles | 8.1 ms | 8.1 ms | 2.7 ms |
| Viscous polar, 37 angles | 658 ms | 508 ms | 487 ms |
| Viscous polar at prescribed Cl, 13 values | 357 ms | 307 ms | 266 ms |
| Viscous polars of 8 airfoils, one core | 5.0 s | 4.2 s | 3.8 s |
| Viscous polars of 8 airfoils, 4 cores | 2.1 s | 1.7 s | 1.6 s |

The same points fail to converge on both sides. In short: per computation,
Aero is as fast as XFOIL compiled with full optimization (0 to 15 % faster)
and about a third faster than XFOIL built with its own Makefile; inviscid
polars are about three times faster (the factorized matrix is reused). The practical gains are
elsewhere: no process to start and no files to parse, results directly in
memory, and the application sweeps many airfoils on all cores at once.

## License

The solver and paneling code are direct ports of XFOIL routines, so the
project is distributed under the **GNU GPL, version 2 or later**, like XFOIL
itself. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
