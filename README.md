# aero

A modern rewrite of [XFOIL](https://web.mit.edu/drela/Public/web/xfoil/)
(Mark Drela) in Rust, with a Tauri desktop application. The goal is a robust,
parallel-friendly airfoil analysis — and later optimization — toolkit that is
usable as a library.

> **Status:** step 1 in progress — geometry, XFOIL-compatible inviscid solver,
> desktop application. Viscous analysis, shape optimization, 3D methods,
> Python bindings and WebAssembly come later.

## Workspace layout

```
crates/
  geometry/     aero-geometry: airfoils, splines, .dat I/O, NACA/CST, paneling
  foil2d/       aero-foil2d:   2D solvers (inviscid panel method; viscous later)
app/            Tauri 2 + Svelte 5 desktop application (thin layer over the crates)
tests/reference XFOIL 6.996 reference results used for validation
docs/           specifications and design notes
```

The computational crates never depend on Tauri and hold no global state.

## Building

Requirements: Rust (the pinned toolchain in `rust-toolchain.toml` is
installed automatically by `rustup`).

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
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

i.e. the results agree to the precision XFOIL prints them. Theoretical
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
