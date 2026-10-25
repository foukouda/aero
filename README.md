# aero

A modern rewrite of [XFOIL](https://web.mit.edu/drela/Public/web/xfoil/)
(Mark Drela) in Rust, with a Tauri desktop application. The goal is a robust,
parallel-friendly airfoil analysis — and later optimization — toolkit that is
usable as a library.

> **Status:** step 1 — geometry, XFOIL-compatible inviscid solver and desktop
> application. Viscous analysis, shape optimization, 3D methods, Python
> bindings and WebAssembly come later; the architecture is prepared for them.

## Workspace layout

```
crates/
  geometry/       aero-geometry: airfoils, splines, .dat I/O, NACA/CST, PANGEN paneling
  foil2d/         aero-foil2d:   2D solvers (XFOIL inviscid panel method; viscous later)
app/
  src-tauri/      aero-app: thin Tauri layer (commands -> Tauri-free engine -> crates)
  src/            Svelte 5 + TypeScript frontend (uPlot charts)
tests/reference/  XFOIL 6.996 reference results used for validation
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
angle-of-attack sweeps with progress and cancellation (Cl(α), Cm(α) for
several airfoils), exports (`.dat`, CSV, PNG, SVG), JSON project files with
a format version, light and dark themes.

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
use aero_foil2d::inviscid::{InviscidOptions, InviscidSolver, OperatingPoint};
use aero_geometry::{NacaOptions, PanelingOptions, naca};

let airfoil = naca("2412", &NacaOptions::default())?.paneled(&PanelingOptions::default())?;
let solver = InviscidSolver::new(&airfoil, &InviscidOptions::default())?; // factorized once
let solution = solver.solve(OperatingPoint::incompressible(4.0))?;        // O(N) per alpha
println!("Cl = {:.4}, Cm = {:.4}", solution.cl, solution.cm);
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

## License

The solver and paneling code are direct ports of XFOIL routines, so the
project is distributed under the **GNU GPL, version 2 or later**, like XFOIL
itself. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
