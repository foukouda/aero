# aero

A modern rewrite of [XFOIL](https://web.mit.edu/drela/Public/web/xfoil/)
(Mark Drela) in Rust, with a Tauri desktop application. The goal is a robust,
parallel-friendly airfoil analysis — and later optimization — toolkit that is
usable as a library.

> **Status:** step 2 — geometry, XFOIL-compatible inviscid and viscous
> solvers (boundary layer, transition, drag, stall) and desktop application.
> Shape optimization, 3D methods, Python bindings and WebAssembly come later;
> the architecture is prepared for them.

## Workspace layout

```
crates/
  geometry/       aero-geometry: airfoils, splines, .dat I/O, NACA/CST, PANGEN paneling
  foil2d/         aero-foil2d:   2D solvers (XFOIL panel method and viscous-inviscid interaction)
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
controls with immediate preview, single angle-of-attack analysis driven by a
live slider (Cp with −Cp up, Cl, Cm, Cp min, optional Mach number with the
Kármán–Tsien correction and a clear error beyond the critical Mach number),
viscous analysis as in XFOIL's `VISC` mode (Reynolds number, Ncrit and
trips per surface; Cl, Cd, L/D, Cm, transition points, pressure and friction
drag; viscous and inviscid Cp; boundary-layer plots of δ*, θ, H, Cf, N and
Ue on both surfaces and the wake), angle-of-attack sweeps with progress and
cancellation (Cl(α), drag polar Cl(Cd), Cm(α) and transition points for
several airfoils; viscous sweeps warm-start each point from the previous one,
as XFOIL's `ASEQ`), exports (`.dat`, CSV of polars, Cp and boundary layer,
PNG, SVG), JSON project files with a format version (older projects are
migrated), light and dark themes, and a guided tour in English and French.

## Libraries and tests

```sh
cargo test --workspace                                 # all Rust tests
cargo clippy --workspace --all-targets -- -D warnings
cargo bench -p aero-foil2d                             # criterion benchmarks
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
history, transition points) for diagnosis.

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
semantics. It is checked against XFOIL 6.996 on ten reference cases
(`tests/viscous`, `crates/foil2d/tests/xfoil_viscous.rs`): NACA 0012 (blunt
and sharp TE, trips, 240 nodes), NACA 2412 (Mach 0.3, different Ncrit on each
side), 4415, 23012, Clark Y and Eppler 387 (laminar separation bubbles), at
Reynolds numbers from 2·10⁵ to 6·10⁶, as polars (warm-started sequences) and
single points:

- the same points converge and fail to converge as in XFOIL (156 converged
  points);
- forces, transition points and x, Cp, θ, δ*, Cf and N or √Cτ at every
  boundary-layer station agree with XFOIL's full-precision dump files to
  10⁻¹⁰ on most points (10⁻⁵ at worst, on a point that needs all its Newton
  iterations);
- XFOIL's console output — the rms change of every Newton iteration, the
  transition points, the marching failures — is reproduced line by line: six
  of the ten sessions are identical over their whole length (thousands of
  lines), the four others until a last printed digit late in a sequence.

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

## License

The solver and paneling code are direct ports of XFOIL routines, so the
project is distributed under the **GNU GPL, version 2 or later**, like XFOIL
itself. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
