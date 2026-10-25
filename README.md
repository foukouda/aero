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

## License

The solver and paneling code are direct ports of XFOIL routines, so the
project is distributed under the **GNU GPL, version 2 or later**, like XFOIL
itself. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
